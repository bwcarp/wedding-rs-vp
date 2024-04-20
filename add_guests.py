#!/usr/bin/python

import csv, mysql.connector

cnx = mysql.connector.connect(user='user', database='rsvp', password="pass", host="127.0.0.1")
cur = cnx.cursor()

guest_insert_plusone = ("INSERT INTO guests"
                        "(id, guest_name, plus_one_allowed, plus_one_name)"
                        "VALUES (%s, %s, %s, %s)")

guest_insert = ("INSERT INTO guests"
                "(id, guest_name, plus_one_allowed)"
                "VALUES (%s, %s, %s)")

with open('guestlist.csv', newline='') as csvfile:
    reader = csv.reader(csvfile)
    for row in reader:
        id = row[0].replace('-','')
        guest_name = row[1]
        plus_one_name = row[3]

        if row[2] == "TRUE":
            plus_one_allowed = True
        else:
            plus_one_allowed = False

        if plus_one_name != "":
            data = (id, guest_name, plus_one_allowed, plus_one_name)
            cur.execute(guest_insert_plusone, data)
        else:
            data = (id, guest_name, plus_one_allowed)
            cur.execute(guest_insert, data)

cnx.commit()