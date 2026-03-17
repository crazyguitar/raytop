# verl Training on Ray

This example runs reinforcement learning from human feedback (RLHF) on a
multi-node Ray cluster using the [verl](https://github.com/verl-project/verl)
framework, deployed on AWS GPU instances (e.g., p4d.24xlarge or p5.48xlarge)
with [Elastic Fabric Adapter (EFA)](https://aws.amazon.com/hpc/efa/) for
high-bandwidth, low-latency inter-node communication. EFA enables NCCL
collectives to run over RDMA rather than TCP, which is critical for the
frequent gradient synchronization and parameter resharding that occur during
on-policy RL training across multiple nodes.

Two training algorithms are provided: Proximal Policy Optimization (PPO) and
Group Relative Policy Optimization (GRPO). Both scripts configure verl to use
the Ray cluster as the distributed backend, handle data loading from Parquet
files, and support arbitrary HuggingFace-compatible model checkpoints.

## Launch Ray Cluster

```bash
salloc -N 4 bash examples/ray/ray.sbatch
# or
sbatch -N 4 examples/ray/ray.sbatch --image /fsx/ray+latest.tar.gz
```

## Run PPO

From inside the head container (`docker exec -it ray-head bash`):

```bash
bash ppo.sh \
  --ray <HEAD_IP>:<RAY_PORT> \
  --model /fsx/models/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B \
  --data /fsx/datasets/gsm8k
```

## Run GRPO

```bash
bash grpo.sh \
  --ray <HEAD_IP>:<RAY_PORT> \
  --model /fsx/models/Qwen/Qwen2.5-3B-Instruct \
  --data /fsx/datasets/gsm8k
```

## Options

| Flag | Description | Default |
|------|-------------|---------|
| `--ray` | Ray cluster address | `127.0.0.1:6379` |
| `--model` | HuggingFace model path | varies by script |
| `--data` | Dataset directory containing `train.parquet` and `test.parquet` | `$PWD/data/gsm8k` |

## Monitor

From any node that can reach the head:

```bash
raytop --master http://<HEAD_IP>:8265
```
