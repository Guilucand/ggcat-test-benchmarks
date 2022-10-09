#!/bin/bash
#
#SBATCH --job-name=article-tests-d-e-f
#SBATCH --output=article-tests-d-e-f.txt
#
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=16
#SBATCH --time=3000:00
#SBATCH --mem=524288

srun ./aurinko-run-benchmarks.sh