#!/bin/bash
set -euo pipefail

# Usage: RAY_ADDRESS=<HEAD_IP>:<RAY_PORT> bash scripts/ppo.sh [--model PATH] [--data DIR]

MODEL="deepseek-ai/deepseek-llm-7b-chat"
WORKSPACE="${WORKSPACE:-$PWD}"
DATA="${WORKSPACE}/data/gsm8k"
RAY_ADDRESS="${RAY_ADDRESS:-127.0.0.1:6379}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model) MODEL="$2"; shift 2 ;;
    --data)  DATA="$2"; shift 2 ;;
    --ray)   RAY_ADDRESS="$2"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

export RAY_ADDRESS

python3 -m verl.trainer.main_ppo \
  algorithm.adv_estimator=gae \
  data.train_files="$DATA/train.parquet" \
  data.val_files="$DATA/test.parquet" \
  data.train_batch_size=1024 \
  data.max_prompt_length=512 \
  data.max_response_length=512 \
  data.filter_overlong_prompts=True \
  data.truncation='error' \
  actor_rollout_ref.model.path="$MODEL" \
  actor_rollout_ref.actor.optim.lr=1e-6 \
  actor_rollout_ref.actor.ppo_mini_batch_size=256 \
  actor_rollout_ref.actor.ppo_micro_batch_size_per_gpu=16 \
  actor_rollout_ref.actor.use_kl_loss=False \
  actor_rollout_ref.actor.fsdp_config.param_offload=False \
  actor_rollout_ref.actor.fsdp_config.optimizer_offload=False \
  actor_rollout_ref.model.use_remove_padding=True \
  actor_rollout_ref.model.enable_gradient_checkpointing=True \
  actor_rollout_ref.rollout.tensor_model_parallel_size=4 \
  actor_rollout_ref.rollout.name=vllm \
  actor_rollout_ref.rollout.gpu_memory_utilization=0.4 \
  actor_rollout_ref.rollout.log_prob_micro_batch_size_per_gpu=32 \
  critic.optim.lr=1e-5 \
  critic.model.path="$MODEL" \
  critic.model.use_remove_padding=True \
  critic.model.enable_gradient_checkpointing=True \
  critic.ppo_micro_batch_size_per_gpu=32 \
  critic.model.fsdp_config.param_offload=False \
  critic.model.fsdp_config.optimizer_offload=False \
  algorithm.use_kl_in_reward=False \
  trainer.critic_warmup=0 \
  trainer.logger='["console"]' \
  trainer.project_name='verl_example_gsm8k' \
  trainer.experiment_name='deepseek_llm_7b_fsdp2' \
  trainer.default_local_dir="${WORKSPACE}/checkpoints" \
  trainer.n_gpus_per_node=8 \
  trainer.nnodes=1 \
  trainer.save_freq=25 \
  trainer.test_freq=1 \
  trainer.use_legacy_worker_impl=auto \
  trainer.total_epochs=10
