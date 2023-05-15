import sys
from pyboolnet import file_exchange, state_transition_graphs, attractors

primes = file_exchange.bnet2primes(sys.stdin.read())
stg = state_transition_graphs.primes2stg(primes, 'asynchronous')
steady, cyclic = attractors.compute_attractors_tarjan(stg)
print(*primes.keys())
print('steady', steady)
print('cyclic', cyclic)
