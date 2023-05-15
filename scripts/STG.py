import sys
import itertools, random
import matplotlib.pyplot as plt
from pyboolnet import file_exchange, state_transition_graphs, state_space

def simulate(primes, semantics_function, sim_length, sims_count):
    result = [{ n: 0 for n in primes.keys() } for _ in range(sim_length)]

    init = list(map(lambda s: dict(zip(primes.keys(), s)),
                    itertools.product((0, 1), repeat=len(primes.keys()))))

    for _ in range(sims_count):
        last = random.choice(init)
        for n, state in last.items():
            result[0][n] += state
        for i in range(1, sim_length):
            last = random.choice(semantics_function(primes, last))
            for n, state in last.items():
                result[i][n] += state

    return [{ n: count / sims_count for n, count in iteration.items()}
            for iteration in result]

def simulate_v1(primes, semantics_function, sim_length):
    sims = []
    sims.append(list(map(lambda s: dict(zip(primes.keys(), s)),
                         itertools.product((0, 1), repeat=len(primes.keys())))))
    for i in range(1, sim_length):
        generation = []
        for net in sims[-1]:
            generation.extend(semantics_function(primes, net))
        sims.append(generation)

    sims_mean = []
    for generation in sims:
        mean = { n: 0 for n in primes.keys() }
        for net in generation:
            for n, s in net.items():
                mean[n] += s
        for n in primes.keys():
            mean[n] /= len(generation)
        sims_mean.append(mean)

    return sims_mean

def IBMFA(primes, sim_length, semantics):
    assert semantics in ['A', 'S']
    tables = dict()
    for n in primes.keys():
        for prime in primes[n][1]:
            tables.setdefault(n, set()).update(
                    state_space.list_states_in_subspace(primes, prime))
    ibmfa = { k: [0.5] for k in primes.keys() }
    for i in range(1, sim_length):
        for n in tables.keys():
            prob = 0
            for row in tables[n]:
                row_prob = 1
                for lit, state in zip(ibmfa.keys(), row):
                    if state == '1':
                        row_prob *= ibmfa[lit][i - 1]
                    else:
                        row_prob *= 1 - ibmfa[lit][i - 1]
                prob += row_prob
            ibmfa[n].append(prob if semantics == 'S' else
                    (prob + (len(primes.keys()) - 1) * ibmfa[n][i - 1])
                    / len(primes.keys()))
    return ibmfa


primes = file_exchange.bnet2primes(sys.stdin.read())
print(primes.keys())
state_transition_graphs.create_stg_image(primes, 'synchronous', 'synchronous.pdf')
state_transition_graphs.create_stg_image(primes, 'asynchronous', 'asynchronous.pdf')
exit(0)

print(primes)

SIM_LENGTH = 10

ibmfa = IBMFA(primes, SIM_LENGTH, 'S')
ax1 = plt.subplot(221)
for n, ps in ibmfa.items():
    ax1.plot(range(len(ps)), ps, label=n)
ax1.legend(loc='upper left')
ax1.set_title('IBMFA synchronous')

ibmfa = IBMFA(primes, 2 * SIM_LENGTH, 'A')
ax2 = plt.subplot(222)
for n, ps in ibmfa.items():
    ax2.plot(range(len(ps)), ps, label=n)
#ax2.legend()
ax2.set_title('IBMFA asynchronous')


#def successors_synchronous(primes, state):
#    return [state_transition_graphs.successor_synchronous(primes, state)]
#
#sims_mean = simulate(primes, successors_synchronous, SIM_LENGTH, 1000)
#ax3 = plt.subplot(223)
#for n in primes.keys():
#    ax3.plot(range(len(sims_mean)), [ g[n] for g in sims_mean ], label=n)
##ax3.legend()
#ax3.set_title('Simulation synchronous')
#
#
#sims_mean = simulate(primes, state_transition_graphs.successors_asynchronous,
#                     SIM_LENGTH, 1000)
#ax2 = plt.subplot(224)
#for n in primes.keys():
#    ax2.plot(range(len(sims_mean)), [ g[n] for g in sims_mean ], label=n)
##ax2.legend()
#ax2.set_title('Simulation asynchronous')


plt.show()
