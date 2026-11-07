Five-stage ring oscillator
* Ring of five inverter stages with assorted element types for analysis tests.
.lib 'models/tech.lib' tt
.include 'cells/inv.inc'

* Parameters, including a user-defined function
.param supply=1.0 wn=200n wp='2*wn' corner=1
.param f2(x)='x*x'
.param warea='f2(wn)'
.global vdd

* One ring stage wraps an inverter cell
.subckt stage in out vdd gnd wn=200n wp=400n
Xinv in out vdd gnd inv wn=wn wp=wp
.ends stage

Vdd vdd 0 'supply'
X1 n1 n2 vdd 0 stage
X2 n2 n3 vdd 0 stage
X3 n3 n4 vdd 0 stage
X4 n4 n5 vdd 0 stage
X5 n5 n1 vdd 0 stage wn=300n wp=600n

* Width sweep table used by the DC sweep below
.data wsweep wn wp
+ 200n 400n
+ 300n 600n
+ 400n 800n
.enddata

.if (corner==1)
.param cload=10f
.else
.param cload2=20f
.endif

* Controlled sources
E1 ea 0 n1 0 2
Ra ea 0 1k
G1 gb 0 VCCS n1 0 1m
Rb gb 0 1k
F1 fc 0 Vsense 2
Rc fc 0 1k
H1 hd 0 Vsense 1k
Rd hd 0 1k
Rs n1 out2 1k
Vsense out2 0 0

* Coupled inductors
L1 n1 0 1n
L2 n2 0 1n
K1 L1 L2 0.9

* Diode and bipolar transistor
D1 n3 0 dmod
.model dmod d is=1e-14
Q1 qc qb qe qmod
.model qmod npn bf=100
Rqc vdd qc 1k
Rqb vdd qb 100k
Rqe qe 0 100

.ic v(n1)=0
.option post
.op
.tran 1p 5n
.dc vdd 0 1 0.1 sweep data=wsweep

* Oscillation period and frequency
.measure tran tpd trig v(n1) val='supply/2' rise=1 targ v(n3) val='supply/2' rise=1
.measure tran fosc param='1/(tpd*10)'
.measure tran vavg avg v(n1) from=1n to=5n
.probe tran v(n1) v(n2) v(n3)

.alter supply at 0.9 V
.param supply=0.9
.end
