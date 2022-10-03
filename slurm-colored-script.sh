#!/bin/bash
#
#SBATCH --job-name=colored-tests
#SBATCH --output=colored-tests.txt
#
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=16
#SBATCH --time=3000:00
#SBATCH --mem=1048576



srun ./run-colored-benchmarks.sh