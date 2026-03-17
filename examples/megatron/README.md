# Megatron Pretrain

This example launches a [Megatron Bridge](https://github.com/NVIDIA-NeMo/Megatron-Bridge)
DeepSeek-V2-Lite pretraining job on a multi-node Ray cluster running on AWS GPU
instances (e.g., p4d.24xlarge or p5.48xlarge) interconnected via
[Elastic Fabric Adapter (EFA)](https://aws.amazon.com/hpc/efa/). EFA provides
low-latency, high-throughput RDMA networking that is essential for the
all-to-all collective operations in expert-parallel MoE training and the
all-reduce traffic in tensor-parallel and data-parallel communication. The
worker environment is pre-configured with the `libfabric` EFA provider and
NCCL tuning variables to ensure that inter-node GPU communication bypasses
the kernel TCP/IP stack and runs over the EFA device directly.

Rather than relying on `torchrun` or MPI-based launchers, the script uses Ray
placement groups to pin one actor per GPU and enforce strict node-local
packing — the same coordination pattern employed by verl's Megatron backend.
Each actor sets up its own `torch.distributed` rank and calls into Megatron
Bridge's `pretrain()` entry point directly, enabling seamless integration with
Ray's scheduling and fault tolerance primitives.

The default configuration trains DeepSeek-V2-Lite with tensor parallelism
(TP=8), expert parallelism (EP=2), and sequence parallelism enabled, which
exercises both the dense attention layers and the Mixture-of-Experts (MoE)
routing and all-to-all communication paths.

## Launch Ray Cluster

```bash
salloc -N 2 bash examples/ray/ray.sbatch --image /fsx/megatron-lm+latest.tar.gz
```

## Run

From inside the head container (`docker exec -it ray-head bash`):

```bash
python3 main.py
python3 main.py --hf-path /fsx/models/deepseek-ai/DeepSeek-V2-Lite
python3 main.py --nodes 2 --gpus-per-node 8
python3 main.py --master-port 29500
```

Or via `ray job submit` from outside the container:

```bash
ray job submit --address http://<HEAD_IP>:8265 -- python3 main.py
```

## Options

| Flag | Description | Default |
|------|-------------|---------|
| `--hf-path` | HuggingFace model path | `/fsx/models/deepseek-ai/DeepSeek-V2-Lite` |
| `--nodes` | Number of nodes | `2` |
| `--gpus-per-node` | GPUs per node | `8` |
| `--master-port` | torch distributed master port | `29500` |

## Monitor

```bash
raytop --master http://<HEAD_IP>:8265
```
