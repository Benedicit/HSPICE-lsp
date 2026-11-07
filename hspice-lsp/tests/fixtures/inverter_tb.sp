CMOS inverter testbench
* Transient testbench for a minimum-size CMOS inverter in a 45nm process.
* Technology models are selected through the library section (corner).
.lib 'models/tech.lib' tt
.include 'cells/inv.inc'

* Design parameters
.param supply=1.0 wn=200n
.param wp='2*wn' cl=10f
.global vdd

* Supply and stimulus
Vdd vdd 0 'supply'
Vin in 0 PULSE(0 'supply' 0 20p 20p 1n 2n)

* Device under test and load
X1 in out vdd 0 inv wn=wn wp=wp
Cload out 0 10f

.option post probe
.tran 1p 4n

* Propagation delay from input falling edge to output rise
.measure tran tphl trig v(in) val='supply/2' rise=1 targ v(out) val='supply/2' fall=1
.probe tran v(in) v(out)
.end
