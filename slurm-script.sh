#!/bin/bash
#
#SBATCH --job-name=assemblers-benchmark1
#SBATCH --output=assemblers-benchmark1.txt
#
#SBATCH --ntasks=1
#SBATCH --cpus-per-task=32
#SBATCH --time=3000:00
#SBATCH --mem=262144



srun ./child-slurm-script.sh