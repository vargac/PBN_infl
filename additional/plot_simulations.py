#!/usr/bin/python3

import sys, json
from matplotlib import pyplot as plt

data = json.load(open(sys.argv[1]))
variables = data['state_variables']
values = data['simulation']

for var in variables:
    plt.plot(range(len(values[var])), values[var], label=var)

plt.gca().set_ylim([-0.05, 1.05])
plt.legend()
plt.show()
