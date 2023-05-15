import sys, json
from matplotlib import pyplot as plt

data = json.load(open(sys.argv[1]))
variables = data['state_variables']
values = data['simulation']

for var in variables:
    plt.plot(range(len(values[var])), values[var], label=var)

gca = plt.gca()
gca.set_ylim([-0.05, 1.05])
gca.set_xlabel('time', fontsize=14)
gca.set_ylabel('probability', fontsize=14)
plt.legend()
plt.show()
