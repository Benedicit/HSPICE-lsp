* CMOS inverter cell, minimum length 45nm
.subckt inv in out vdd gnd wn=200n wp=400n
M1 out in gnd gnd nch W=wn L=45n
M2 out in vdd vdd pch W=wp L=45n
.ends inv
