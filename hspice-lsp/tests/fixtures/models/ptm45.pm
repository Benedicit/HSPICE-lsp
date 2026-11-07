* PTM 45nm high-performance style BSIM4 model cards (binned)

.model nch.1 nmos level=54
+ lmin=4.5e-8 lmax=1e-7 wmin=9e-8 wmax=1e-6
+ version = 4.0 binunit = 1 paramchk = 1 mobmod = 0
+ capmod = 2 igcmod = 1 igbmod = 1 geomod = 1 diomod = 1 rdsmod = 0
+ rgatemod = 0 permod = 1 acnqsmod = 0 trnqsmod = 0
+ tnom = 27 toxe = 1.25e-9 toxp = 1e-9 toxm = 1.25e-9 dtox = 0.25e-9
+ epsrox = 3.9 wint = 5e-009 lint = 3.75e-009 ll = 0 wl = 0
+ lln = 1 wln = 1 lw = 0 ww = 0 lwn = 1 wwn = 1 lwl = 0 wwl = 0
+ xl = -20e-9 xw = -20e-9 dlc = 3.75e-009 dwc = 5e-9
* threshold voltage and short channel effect parameters
+ vth0 = 0.46893 k1 = 0.4 k2 = 0.01 k3 = 0 k3b = 0 w0 = 2.5e-006
+ dvt0 = 1 dvt1 = 2 dvt2 = -0.032 dvt0w = 0 dvt1w = 0 dvt2w = 0
+ dsub = 0.1 minv = 0.05 voffl = 0 dvtp0 = 1e-009 dvtp1 = 0.1
+ lpe0 = 0 lpeb = 0 xj = 1.4e-008 ngate = 1e+023 ndep = 1.94e+018
+ nsd = 2e+020 phin = 0 cdsc = 0.000 cdscb = 0 cdscd = 0 cit = 0
+ voff = -0.13 nfactor = 2.1 eta0 = 0.0055 etab = 0
+ u0 = 0.04708 ua = 6e-010 ub = 1.2e-018 uc = 0 vsat = 147000
+ a0 = 1.0 ags = 1e-020 a1 = 0 a2 = 1 b0 = -1e-020 b1 = 0
+ keta = 0.04 dwg = 0 dwb = 0 pclm = 0.04 pdiblc1 = 0.001
+ pdiblc2 = 0.001 pdiblcb = -0.005 drout = 0.5 pvag = 1e-020
+ delta = 0.01 pscbe1 = 8.14e+008 pscbe2 = 1e-007
+ fprout = 0.2 pdits = 0.08 pditsd = 0.23 pditsl = 2300000
+ rsh = 5 rdsw = 155 rsw = 75 rdw = 75 wr = 1 prwg = 0 prwb = 0
+ cgso = 1.1e-010 cgdo = 1.1e-010 cgbo = 2.56e-011 cgsl = 0 cgdl = 0
+ ckappas = 0.03 ckappad = 0.03 acde = 1 moin = 15 noff = 0.9 voffcv = 0.02
* junction and temperature parameters
+ cj = 0.0005 mj = 0.5 pb = 1 cjsw = 5e-010 mjsw = 0.33 pbsw = 1
+ cjswg = 5e-010 mjswg = 0.33 pbswg = 1 js = 0.0001 jsw = 0
+ ute = -1.5 kt1 = -0.11 kt1l = 0 kt2 = 0.022 ua1 = 4.31e-009
+ ub1 = 7.61e-018 uc1 = -5.6e-011 at = 33000 prt = 0

.model nch.2 nmos level=54
+ lmin=1e-7 lmax=1e-6 wmin=9e-8 wmax=1e-6
+ version = 4.0 binunit = 1 paramchk = 1 mobmod = 0
+ capmod = 2 igcmod = 1 igbmod = 1 geomod = 1 diomod = 1 rdsmod = 0
+ tnom = 27 toxe = 1.25e-9 toxp = 1e-9 toxm = 1.25e-9 dtox = 0.25e-9
+ vth0 = 0.4426 k1 = 0.45 k2 = 0.02 dvt0 = 1.2 dvt1 = 1.8 dvt2 = -0.03
+ dsub = 0.2 xj = 1.4e-008 ngate = 1e+023 ndep = 1.7e+018
+ voff = -0.12 nfactor = 1.9 eta0 = 0.0045 etab = 0
+ u0 = 0.0521 ua = 5.5e-010 ub = 1.1e-018 uc = 0 vsat = 152000
+ pclm = 0.05 pdiblc1 = 0.002 pdiblc2 = 0.001 drout = 0.5
+ rdsw = 150 cgso = 1.1e-010 cgdo = 1.1e-010 cgbo = 2.56e-011
+ cj = 0.0005 mj = 0.5 pb = 1 cjsw = 5e-010 mjsw = 0.33 pbsw = 1
+ js = 0.0001 jsw = 0 ute = -1.5 kt1 = -0.11 kt2 = 0.022

.model pch.1 pmos level=54
+ lmin=4.5e-8 lmax=1e-7 wmin=9e-8 wmax=1e-6
+ version = 4.0 binunit = 1 paramchk = 1 mobmod = 0
+ capmod = 2 igcmod = 1 igbmod = 1 geomod = 1 diomod = 1 rdsmod = 0
+ tnom = 27 toxe = 1.3e-9 toxp = 1e-9 toxm = 1.3e-9 dtox = 0.3e-9
+ epsrox = 3.9 wint = 5e-009 lint = 3.75e-009 xl = -20e-9 xw = -20e-9
+ vth0 = -0.43121 k1 = 0.4 k2 = -0.01 k3 = 0 w0 = 2.5e-006
+ dvt0 = 1 dvt1 = 2 dvt2 = -0.032 dsub = 0.1 minv = 0.05
+ xj = 1.4e-008 ngate = 1e+023 ndep = 1.2e+018 nsd = 2e+020
+ voff = -0.126 nfactor = 2.1 eta0 = 0.0055 etab = 0
+ u0 = 0.00453 ua = 2e-009 ub = 5e-019 uc = 0 vsat = 70000
+ a0 = 1.0 ags = 1e-020 keta = -0.047 pclm = 0.12 pdiblc1 = 0.001
+ pdiblc2 = 0.001 drout = 0.5 pscbe1 = 8.14e+008 pscbe2 = 9.58e-007
+ rsh = 5 rdsw = 155 prwg = 0 prwb = 0
+ cgso = 1.1e-010 cgdo = 1.1e-010 cgbo = 2.56e-011
+ cj = 0.0005 mj = 0.5 pb = 1 cjsw = 5e-010 mjsw = 0.33 pbsw = 1
+ js = 0.0001 jsw = 0 ute = -1.5 kt1 = -0.11 kt2 = 0.022
+ ua1 = 4.31e-009 ub1 = 7.61e-018 uc1 = -5.6e-011 at = 33000

.model pch.2 pmos level=54
+ lmin=1e-7 lmax=1e-6 wmin=9e-8 wmax=1e-6
+ version = 4.0 binunit = 1 paramchk = 1 mobmod = 0
+ capmod = 2 igcmod = 1 igbmod = 1 geomod = 1 diomod = 1 rdsmod = 0
+ tnom = 27 toxe = 1.3e-9 toxp = 1e-9 toxm = 1.3e-9 dtox = 0.3e-9
+ vth0 = -0.4106 k1 = 0.45 k2 = -0.02 dvt0 = 1.2 dvt1 = 1.8 dvt2 = -0.03
+ dsub = 0.2 xj = 1.4e-008 ngate = 1e+023 ndep = 1.1e+018
+ voff = -0.12 nfactor = 1.9 eta0 = 0.0045 etab = 0
+ u0 = 0.0049 ua = 1.9e-009 ub = 4.8e-019 uc = 0 vsat = 75000
+ pclm = 0.14 pdiblc1 = 0.002 pdiblc2 = 0.001 drout = 0.5
+ rdsw = 150 cgso = 1.1e-010 cgdo = 1.1e-010 cgbo = 2.56e-011
+ cj = 0.0005 mj = 0.5 pb = 1 cjsw = 5e-010 mjsw = 0.33 pbsw = 1
+ js = 0.0001 jsw = 0 ute = -1.5 kt1 = -0.11 kt2 = 0.022
