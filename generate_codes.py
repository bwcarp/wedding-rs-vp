#!/usr/bin/python3

import random, string, re

chars = string.ascii_uppercase + string.digits
codes = set()

while len(codes) < 57:
    code = ''.join(random.choice(chars) for i in range(12))
    code = '-'.join(re.findall('....', code))
    codes.add(code)

for code in codes:
    print(code)
