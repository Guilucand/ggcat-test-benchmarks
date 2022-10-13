#!/bin/bash
#
#SBATCH --job-name=article-tests-g
#SBATCH --output=article-tests-g.txt
#SBATCH --cpus-per-task=200
#SBATCH --ntasks=1
#SBATCH --time=3000:00
#SBATCH --mem=524288

for threads in {192,128,64,32,16,8,4,2,1}; do
    srun --cpus-per-task $threads ./aurinko-single-run-test-g.sh $threads
done