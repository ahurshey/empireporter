# SPDX-License-Identifier: GPL-3.0-only
#
# Copyright (C) 2026 Alex Hurshman
#
# This file is part of EmpirePorter.
#
# EmpirePorter is free software: you can redistribute it and/or modify it under
# the terms of the GNU General Public License as published by the Free Software
# Foundation, version 3 only.
#
# EmpirePorter is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

import re
import os.path

pattern = re.compile(r'"([a-zA-Z0-9\s]+)"=')
civFilePath = ""
civs = dict()
civNum = 0
exportChoice = 0

while os.path.isfile(civFilePath) == False:
    civFilePath = input('Enter path to user empire designs file: ')

totalLines = sum(1 for _ in open(civFilePath))

for i, line in enumerate(open(civFilePath)):
    for match in re.finditer(pattern, line):
        civNum += 1
        civName = match.group(1)
        civ = {
            "name": civName,
            "start": i
            }
        civs[civNum] = civ
        if civNum > 1:
            civs[civNum-1]["end"] = i

civs[len(civs)]["end"] = totalLines

print("Found Empires: ")
for civ in civs:
    print(civ, ":", civs[civ]["name"])

while int(exportChoice) not in range(1, len(civs)):
    exportChoice = input('Enter number to export empire: ')

print("Exporting to: ", civs[int(exportChoice)]["name"], ".txt")

with open(civFilePath,'r') as civFile, open(civs[int(exportChoice)]["name"] + ".txt",'a') as outFile:
    for i, line in enumerate(civFile):
        if i in range(int(civs[int(exportChoice)]["start"]), int(civs[int(exportChoice)]["end"])): 
         outFile.write(line)
print("Done!")
