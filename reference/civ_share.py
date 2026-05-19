import re
import os.path

pattern = re.compile("\"([a-zA-Z0-9\\s]+)\"\=")
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
