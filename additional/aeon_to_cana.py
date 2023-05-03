import sys

influencer, influenced = set(), set()
rules = list()

for line in sys.stdin:
    line = line.strip()
    if line[0] == '#':
        continue

    if line[0] == '$':
        line = line[1:]
        new_line = (line
            .replace('|', 'or')
            .replace('&', 'and')
            .replace('!', 'not ')
            .replace(':', '*='))
        rules.append(new_line)
    else:
        splitted = line.split()
        influencer.add(splitted[0])
        influenced.add(splitted[-1])

for control in influencer - influenced:
    print(control, '*=', control)
for rule in rules:
    print(rule)
