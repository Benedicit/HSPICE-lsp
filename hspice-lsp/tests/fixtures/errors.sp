Netlist with one example of each diagnostic
* Each faulty construct is annotated with "$ expect: <code>" on its line,
* or "* expect-anywhere: <code>" when the location is not a single line.

* backbone so that n1 is never floating
Vbias n1 0 1
Rbb n1 0 1k
L1 n1 0 1n

* helper subcircuits (valid)
.subckt two a b
Rtwo1 a b 1k
Rtwo2 a b 2k
.ends two
.subckt withp a b gain=1
Rg1 a b 'gain*1k'
Rg2 a b 5k
.ends withp
.model dm d is=1e-14

* case: unmatched-end
.endl stray $ expect: unmatched-end

* case: syntax-error (unbalanced parenthesis)
Vsyn n1 0 PULSE(0 1 0 1n 1n $ expect: syntax-error

* case: unknown-element
Y1 n1 0 1k $ expect: unknown-element

* case: missing-nodes
R5 n1 $ expect: missing-nodes

* case: undefined-param
R2 n1 0 'rmissing*2' $ expect: undefined-param

* case: undefined-function
Rf n1 0 'foo(1)' $ expect: undefined-function

* case: function-arity
Rm n1 0 'max(1)' $ expect: function-arity

* case: undefined-subckt
Xu1 n1 0 nosuchsub $ expect: undefined-subckt

* case: port-count (subckt two has 2 ports)
Xp n1 0 n1 two $ expect: port-count

* case: unknown-instance-param (gain is declared, zz is not)
Xq n1 0 withp zz=1 $ expect: unknown-instance-param

* case: undefined-model
M1 n1 n1 0 0 nch_missing W=1u L=1u $ expect: undefined-model

* case: model-type-mismatch (MOSFET using a diode model)
M2 n1 n1 0 0 dm W=1u L=1u $ expect: model-type-mismatch

* case: undefined-source
F2 n1 0 Vnone 2 $ expect: undefined-source

* case: undefined-inductor
K2 L1 Lnone 0.5 $ expect: undefined-inductor

* case: include-not-found (inside a wrapper subckt, so the top-level floating-node check stays active)
.subckt incwrap a
.include 'does/not/exist.inc' $ expect: include-not-found
.ends incwrap

* case: duplicate-element
* expect-anywhere: duplicate-element
R3 n1 0 1k
R3 n1 0 2k

* case: analysis-args
.tran 1n $ expect: analysis-args

* case: reserved-param
.param temper=27 $ expect: reserved-param

* case: bad-number (exponent combined with scale factor)
Rbn n1 0 1e-6u $ expect: bad-number

* case: meg-suffix (M means milli)
R4 n1 0 10M $ expect: meg-suffix

* case: floating-node (lonely is used only once)
Rfl n1 lonely 1k $ expect: floating-node

* case: duplicate-param
* expect-anywhere: duplicate-param
.param dp=1
.param dp=2

* case: unquoted-expression
.param negbase=1
.param neg=-negbase $ expect: unquoted-expression

* case: duplicate-model
* expect-anywhere: duplicate-model
.model dupm d is=1e-14
.model dupm d is=2e-14

* case: duplicate-subckt
* expect-anywhere: duplicate-subckt
.subckt dup a b
Rdup1 a b 1k
Rdup2 a b 2k
.ends dup
.subckt dup a b
Rdup3 a b 3k
Rdup4 a b 4k
.ends dup

* case: unknown-command
.foo $ expect: unknown-command

* case: undefined-node and undefined-element in output variables
.probe v(nosuch) $ expect: undefined-node
.probe i(rnosuch) $ expect: undefined-element

* case: mosra-args and unknown-mosra-param
.mosra relstep=1 $ expect: mosra-args
.mosra reltotaltime=1 bogusparam=1 $ expect: unknown-mosra-param

* case: mosra-sweep and mosra-dc-ac (the .mosra commands above run the stress phase)
.tran 1n 10n sweep monte=3 $ expect: mosra-sweep
.dc Vbias 0 1 0.1 $ expect: mosra-dc-ac

* case: bad-port (ground in port list)
.subckt bp a 0 $ expect: bad-port
Rbp1 a 0 1k
Rbp2 a 0 2k
.ends bp

* case: ends-name-mismatch
.subckt nm a b
Rnm1 a b 1k
Rnm2 a b 2k
.ends other $ expect: ends-name-mismatch

* case: shorted-element, missing-value, bad-multiplier, large-value
Rsh n1 n1 1k $ expect: shorted-element
Rmv n1 0 $ expect: missing-value
Rbm n1 0 1k m=0 $ expect: bad-multiplier
Clv n1 0 1 $ expect: large-value

* case: source-args (PWL time goes backwards)
Vsa n1 0 PWL(0 0 2n 1 1n 0) $ expect: source-args

* case: undefined-data
.tran data=nodata $ expect: undefined-data

* case: duplicate-ic and empty-command
.ic v(n1)=0
.ic v(n1)=1 $ expect: duplicate-ic
.nodeset $ expect: empty-command

* case: invalid-value and runlvl-ignored-option
.option method=euler $ expect: invalid-value
.option trtol=7 $ expect: runlvl-ignored-option

* case: output-signal-param
.param osp='v(n1)*2' $ expect: output-signal-param
Rosp n1 0 'osp+1k'

* case: measure-keyword-param
.param td=1n
Rtd n1 0 'td*1e12'
.meas tran mkp avg v(n1) from=0 to=td $ expect: measure-keyword-param

* case: misplaced-else
.if (1)
.else
.elseif (0) $ expect: misplaced-else
.endif

* case: mosra-model-in-alter (every later case lies in this .alter section)
.alter aged
.model nra mosra level=1 $ expect: mosra-model-in-alter

* case: unclosed-block (no .ends before .end)
.subckt unclosed a b $ expect: unclosed-block
Ruc1 a b 1k
Ruc2 a b 2k
.end
