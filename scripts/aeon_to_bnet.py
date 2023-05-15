import sys

act_by = dict()
inh_by = dict()
all_ = set()
rules = list()
ruled = set()
unknown_arrows = set()

for line in sys.stdin:
    line = line.strip()
    if line[0] == '#':
        continue

    if line[0] == '$':
        line = line[1:]
        new_line = (line
            .replace(':', ', '))
        rules.append(new_line)
        ruled.add(new_line.split(',')[0])
    else:
        fr, arrow, to = line.split()
        if arrow == '->':
            act_by.setdefault(to, set()).add(fr)
        elif arrow == '-|':
            inh_by.setdefault(to, set()).add(fr)
        else:
            unknown_arrows.add(to)
        all_.add(fr)
        all_.add(to)

if unknown_arrows - ruled:
    assert False

infl = set(act_by.keys()) | set(inh_by.keys())

for node in all_ - ruled:
    if node in infl:
        if len(act_by.get(node, [])) == 1 and len(inh_by.get(node, [])) == 0:
            print(node, ', ', act_by[node].copy().pop())
        elif len(act_by.get(node, [])) == 0 and len(inh_by.get(node, [])) == 1:
            print(node, ', ', '!' + inh_by[node].copy().pop())
        else:
            assert False
    else:
        print(node, ', ', node)
for rule in rules:
    print(rule)
