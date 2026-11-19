* Data-driven sweeps with .DATA lines written without '+' (HSPICE Commands manual E-2010.12, .DATA examples)
.PARAM VDS=0 VBS=0 L=1.0u W1=50u W2=50u CAP=0 p1=0 p2=0 p3=0
M1 1 2 3 0 nch W=W1 L=L
VGS 2 0 0.0v
VBS 3 0 VBS
VDS 1 0 VDS
C1 1 0 CAP
G1 1 0 noise='TABLE(HERTZ, dnoise)'
.MODEL nch NMOS LEVEL=1

.DC DATA=vdot
.TRAN 1n 100n SWEEP DATA=d1

* Column names on their own line, then one row of values per line
.DATA vdot
VBS VDS L
0 0.1 1.5u
 0 0.1 1.0u
 -1 0.1 1.0u
.ENDDATA

.DATA d1
W1     W2    L      CAP
50u    40u   1.0u   1.2pf
25u    20u   0.8u   0.9pf
.ENDDATA

* Noise table for TABLE() (User Guide H-2013.03)
.DATA dnoise
+ f v
+ 1k 1e-9
+ 1meg 1e-12
.ENDDATA

* Concatenated external files
.DATA merged MER
FILE=file1 p1=1 p2=3
FILE=file2 p3=1
.ENDDATA
.END
