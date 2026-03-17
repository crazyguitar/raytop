#!/usr/bin/env python3
"""Launch Megatron Bridge DeepSeek-V2-Lite pretrain on a Ray cluster.

Uses Ray placement groups to pin workers to nodes (same pattern as verl's
Megatron backend). Each Ray actor = one GPU rank, calling
torch.distributed.init_process_group() directly — no torchrun needed.

Usage:
    # Start Ray cluster first:
    salloc -N 2 bash examples/ray/ray.sbatch --image /fsx/megatron-lm+latest.tar.gz

    # Then from inside the head container:
    python3 main.py
    python3 main.py --hf-path /fsx/models/deepseek-ai/DeepSeek-V2-Lite
    python3 main.py --nodes 2 --gpus-per-node 8
"""

import argparse
import os

import ray
from ray.util.placement_group import placement_group
from ray.util.scheduling_strategies import PlacementGroupSchedulingStrategy


# ── EFA / NCCL environment ──────────────────────────────────────────────────

EFA_ENV = {
    "FI_PROVIDER": "efa",
    "FI_EFA_USE_DEVICE_RDMA": "1",
    "FI_EFA_FORK_SAFE": "1",
    "NCCL_DEBUG": "WARN",
    "NCCL_BUFFSIZE": "8388608",
    "NCCL_P2P_NET_CHUNKSIZE": "524288",
    "CUDA_DEVICE_MAX_CONNECTIONS": "1",
    "OMP_NUM_THREADS": "1",
}


# ── Recipe ───────────────────────────────────────────────────────────────────


def build_config(hf_path=None):
    from megatron.bridge.recipes.deepseek.deepseek_v2 import (
        deepseek_v2_lite_pretrain_config,
    )

    cfg = deepseek_v2_lite_pretrain_config(
        **({"hf_path": hf_path} if hf_path else {}),
        tensor_model_parallel_size=8,
        pipeline_model_parallel_size=1,
        expert_model_parallel_size=2,
        sequence_parallel=True,
        seq_length=4096,
        train_iters=500,
        global_batch_size=64,
        micro_batch_size=1,
        eval_interval=100,
        lr_warmup_iters=50,
        save_interval=0,
    )
    cfg.model.moe_permute_fusion = False
    return cfg


# ── Worker ───────────────────────────────────────────────────────────────────


@ray.remote(num_cpus=0, num_gpus=1)
class Worker:
    def run(self, rank, local_rank, world_size, master_addr, master_port, hf_path):
        # Must reset CUDA_VISIBLE_DEVICES BEFORE any torch/CUDA import
        gpu_ids = ray.get_gpu_ids()
        cuda_device = int(gpu_ids[0]) if gpu_ids else local_rank
        os.environ.pop("CUDA_VISIBLE_DEVICES", None)

        os.environ.update(EFA_ENV)
        os.environ.update(
            {
                "RANK": str(rank),
                "LOCAL_RANK": str(cuda_device),
                "WORLD_SIZE": str(world_size),
                "MASTER_ADDR": master_addr,
                "MASTER_PORT": str(master_port),
            }
        )

        import megatron.core.jit as _jit

        if not hasattr(_jit, "disable_jit_fuser"):
            _jit.disable_jit_fuser = lambda: None

        from megatron.bridge.training.gpt_step import forward_step
        from megatron.bridge.training.pretrain import pretrain

        cfg = build_config(hf_path)
        pretrain(config=cfg, forward_step_func=forward_step)


# ── Placement ────────────────────────────────────────────────────────────────


def create_placement_groups(num_nodes, gpus_per_node):
    """Create one placement group per node, each with gpus_per_node GPU bundles."""
    pgs = []
    for _ in range(num_nodes):
        bundles = [{"GPU": 1} for _ in range(gpus_per_node)]
        pg = placement_group(bundles, strategy="STRICT_PACK")
        ray.get(pg.ready())
        pgs.append(pg)
    return pgs


def get_pg_node_ip(pg):
    """Get the node IP where a placement group is located."""
    return pg.bundle_specs[0].get("node:__internal_head__", None) or ray.get(
        ray.remote(num_gpus=0)
        .options(
            scheduling_strategy=PlacementGroupSchedulingStrategy(
                placement_group=pg,
                placement_group_bundle_index=0,
            )
        )
        .remote(lambda: ray.util.get_node_ip_address())
    )


def resolve_master_and_sort_pgs(pgs):
    """Resolve node IPs for each PG, sort by IP, return (sorted_pgs, master_addr)."""

    @ray.remote(num_cpus=0)
    def _get_ip():
        return ray.util.get_node_ip_address()

    # Schedule a tiny task on each PG to discover its node IP
    ip_futures = []
    for pg in pgs:
        ref = _get_ip.options(
            scheduling_strategy=PlacementGroupSchedulingStrategy(
                placement_group=pg,
                placement_group_bundle_index=0,
            )
        ).remote()
        ip_futures.append(ref)

    ips = ray.get(ip_futures)
    pg_ip_pairs = sorted(zip(pgs, ips), key=lambda x: x[1])
    sorted_pgs = [p for p, _ in pg_ip_pairs]
    master_addr = pg_ip_pairs[0][1]
    return sorted_pgs, master_addr


# ── Main ─────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--hf-path", default="deepseek-ai/DeepSeek-V2-Lite")
    parser.add_argument("--nodes", type=int, default=2)
    parser.add_argument("--gpus-per-node", type=int, default=8)
    parser.add_argument("--master-port", type=int, default=29500)
    args = parser.parse_args()

    world_size = args.nodes * args.gpus_per_node

    ray.init()
    print(f"Creating {args.nodes} placement groups ({args.gpus_per_node} GPUs each)...")
    pgs = create_placement_groups(args.nodes, args.gpus_per_node)
    sorted_pgs, master_addr = resolve_master_and_sort_pgs(pgs)
    print(f"Ray: {args.nodes} nodes, master={master_addr}, world_size={world_size}")

    # Spawn one worker per GPU, pinned to placement groups
    futures = []
    rank = 0
    for pg in sorted_pgs:
        for local_rank in range(args.gpus_per_node):
            worker = Worker.options(
                scheduling_strategy=PlacementGroupSchedulingStrategy(
                    placement_group=pg,
                    placement_group_bundle_index=local_rank,
                )
            ).remote()
            futures.append(
                worker.run.remote(
                    rank,
                    local_rank,
                    world_size,
                    master_addr,
                    args.master_port,
                    args.hf_path,
                )
            )
            rank += 1

    ray.get(futures)


if __name__ == "__main__":
    main()
