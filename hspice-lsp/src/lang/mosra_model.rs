//! Level 1 MOSRA (HCI/BTI reliability) model-card parameters, HSPICE User Guide H-2013.03 tables 70-72.

use super::models::ModelParamSpec;

/// Parameters of `.model name MOSRA LEVEL=1` cards.
pub static MOSRA_MODEL_PARAMS: &[ModelParamSpec] = &[
    ModelParamSpec {
        name: "level",
        default: Some("1"),
        description: "MOSRA model level: 1 selects the Synopsys model, other values a custom MOSRA API model",
    },
    ModelParamSpec {
        name: "mosralevel",
        default: Some("1"),
        description: "Alias of LEVEL (HSIM compatibility)",
    },
    ModelParamSpec {
        name: "relmode",
        default: Some("0"),
        description: "Degradation effects of this model: 0 HCI and BTI, 1 HCI only, 2 BTI only (.MOSRA RelMode=1|2 overrides)",
    },
    ModelParamSpec {
        name: "tit0",
        default: Some("0"),
        description: "First parameter for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "titce",
        default: Some("0"),
        description: "Inversion charge exponent for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "titga",
        default: Some("0"),
        description: "Vgs dependence offset for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "titfd",
        default: Some("0"),
        description: "Oxide electric field dependence for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tittd",
        default: Some("0"),
        description: "Temperature dependent component of interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "titwc",
        default: Some("0"),
        description: "Channel width coefficient for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "titwe",
        default: Some("0"),
        description: "Channel width exponent for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "titlc",
        default: Some("0"),
        description: "Channel length coefficient for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "title",
        default: Some("0"),
        description: "Channel length exponent for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tn",
        default: Some("0.25"),
        description: "Stress time exponent for interface-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tot0",
        default: Some("0"),
        description: "First parameter for oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "totfd",
        default: Some("0"),
        description: "Oxide electric field dependent component of oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tottd",
        default: Some("0"),
        description: "Temperature dependent component of oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "totdd",
        default: Some("0"),
        description: "Drain voltage dependent coefficient for the oxide electric field in Vth degradation",
    },
    ModelParamSpec {
        name: "totwc",
        default: Some("0"),
        description: "Channel width coefficient of oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "totwe",
        default: Some("0"),
        description: "Channel width exponent of oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "totlc",
        default: Some("0"),
        description: "Channel length coefficient of oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "totle",
        default: Some("0"),
        description: "Channel length component of oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tk",
        default: Some("0.5"),
        description: "Stress time exponent for oxide-trap-induced Vth degradation",
    },
    ModelParamSpec {
        name: "ttd0",
        default: Some("1"),
        description: "First parameter for transient Vth degradation (0 disables Vth recovery)",
    },
    ModelParamSpec {
        name: "tdcd",
        default: Some("0"),
        description: "Duty cycle dependent exponent for transient Vth degradation (0 or negative)",
    },
    ModelParamSpec {
        name: "tfc",
        default: Some("0"),
        description: "Frequency dependence coefficient for BTI recovery of Vth",
    },
    ModelParamSpec {
        name: "tfe",
        default: Some("0"),
        description: "Frequency dependence exponent for BTI recovery of Vth",
    },
    ModelParamSpec {
        name: "totde",
        default: Some("1"),
        description: "Drain voltage exponent for the oxide electric field in Vth degradation",
    },
    ModelParamSpec {
        name: "eoxmod",
        default: Some("0"),
        description: "1 enables separate electric field equations for mobility, DIBL and VSAT degradation",
    },
    ModelParamSpec {
        name: "dlbti",
        default: Some("0"),
        description: "Length dependence offset for the BTI model",
    },
    ModelParamSpec {
        name: "dwbti",
        default: Some("0"),
        description: "Width dependence offset for the BTI model",
    },
    ModelParamSpec {
        name: "lnomra",
        default: Some("0"),
        description: "Nominal length for NBTI length dependence limiting",
    },
    ModelParamSpec {
        name: "uit0",
        default: Some("0"),
        description: "First parameter for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitce",
        default: Some("0"),
        description: "Inversion charge exponent for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitga",
        default: Some("0"),
        description: "Vgs dependence offset for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitfd",
        default: Some("0"),
        description: "Oxide electric field dependence for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uittd",
        default: Some("0"),
        description: "Temperature dependent coefficient of interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitwc",
        default: Some("0"),
        description: "Channel width coefficient for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitwe",
        default: Some("0"),
        description: "Channel width exponent for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitlc",
        default: Some("0"),
        description: "Channel length coefficient for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uitle",
        default: Some("0"),
        description: "Channel length exponent for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "un",
        default: Some("0.25"),
        description: "Stress time exponent for interface-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uot0",
        default: Some("0"),
        description: "First parameter for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uotfd",
        default: Some("0"),
        description: "Oxide electric field dependence for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uottd",
        default: Some("0"),
        description: "Temperature dependence for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uotwc",
        default: Some("0"),
        description: "Channel width coefficient for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uotwe",
        default: Some("0"),
        description: "Channel width exponent for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uotlc",
        default: Some("0"),
        description: "Channel length coefficient for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uotle",
        default: Some("0"),
        description: "Channel length exponent for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uk",
        default: Some("0.5"),
        description: "Stress time exponent for oxide-trap-induced mobility degradation",
    },
    ModelParamSpec {
        name: "utd0",
        default: Some("1"),
        description: "First parameter for transient mobility degradation (0 disables mobility recovery)",
    },
    ModelParamSpec {
        name: "udcd",
        default: Some("0"),
        description: "Duty cycle dependent coefficient for transient mobility degradation (0 or negative)",
    },
    ModelParamSpec {
        name: "ufc",
        default: Some("0"),
        description: "Frequency dependence coefficient for BTI recovery of mobility",
    },
    ModelParamSpec {
        name: "ufe",
        default: Some("0"),
        description: "Frequency dependence exponent for BTI recovery of mobility",
    },
    ModelParamSpec {
        name: "uotdd",
        default: Some("TOTDD"),
        description: "Drain voltage dependent coefficient for the oxide electric field in mobility degradation (EOXMOD=1)",
    },
    ModelParamSpec {
        name: "uotde",
        default: Some("TOTDE"),
        description: "Drain voltage exponent for the oxide electric field in mobility degradation (EOXMOD=1)",
    },
    ModelParamSpec {
        name: "thci0",
        default: Some("0"),
        description: "First parameter for HCI-induced Vth degradation",
    },
    ModelParamSpec {
        name: "thci1",
        default: Some("0"),
        description: "Coefficient for the Isub-dependent term of HCI degradation",
    },
    ModelParamSpec {
        name: "uhci0",
        default: Some("0"),
        description: "First parameter for HCI-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uhci1",
        default: Some("0"),
        description: "Coefficient for the Isub-dependent term of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "tdii",
        default: Some("0"),
        description: "Impact ionization current exponent for HCI-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tdle",
        default: Some("1"),
        description: "Channel length exponent for HCI-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tdce",
        default: Some("0"),
        description: "Channel current exponent for HCI-induced Vth degradation",
    },
    ModelParamSpec {
        name: "tea",
        default: Some("0"),
        description: "Equivalent activation energy of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "tdlt",
        default: Some("0"),
        description: "Channel length dependence term of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "thcvd",
        default: Some("0"),
        description: "Coefficient for the Vds-dependent term of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "tdvd",
        default: Some("0"),
        description: "Vds term exponent of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "tdid",
        default: Some("1"),
        description: "Channel current exponent for the Vds-dependent term of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "thcvb",
        default: Some("0"),
        description: "Coefficient for the Vbs dependence of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "tdvb",
        default: Some("1"),
        description: "Exponent for the Vbs dependence of Vth HCI degradation",
    },
    ModelParamSpec {
        name: "hn",
        default: Some("0.5"),
        description: "Time exponent for HCI-induced Vth degradation",
    },
    ModelParamSpec {
        name: "udce",
        default: Some("0"),
        description: "Channel current exponent for HCI-induced mobility degradation",
    },
    ModelParamSpec {
        name: "udii",
        default: Some("0"),
        description: "Impact ionization current exponent for HCI-induced mobility degradation",
    },
    ModelParamSpec {
        name: "udle",
        default: Some("1"),
        description: "Channel length exponent for HCI-induced mobility degradation",
    },
    ModelParamSpec {
        name: "uea",
        default: Some("0"),
        description: "Equivalent activation energy of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "udlt",
        default: Some("0"),
        description: "Channel length dependence term of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "uhcvd",
        default: Some("0"),
        description: "Coefficient for the Vds-dependent term of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "udvd",
        default: Some("0"),
        description: "Vds term exponent of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "udid",
        default: Some("1"),
        description: "Channel current exponent for the Vds-dependent term of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "uhcvb",
        default: Some("0"),
        description: "Coefficient for the Vbs dependence of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "udvb",
        default: Some("1"),
        description: "Exponent for the Vbs dependence of mobility HCI degradation",
    },
    ModelParamSpec {
        name: "hk",
        default: Some("0.5"),
        description: "Time exponent for HCI-induced mobility degradation",
    },
    ModelParamSpec {
        name: "hiii",
        default: Some("0"),
        description: "First parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivd0",
        default: Some("0"),
        description: "First Vds dependent parameter of the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivd1",
        default: Some("0"),
        description: "Second Vds dependent parameter of the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivd2",
        default: Some("0"),
        description: "Third Vds dependent parameter of the impact ionization current",
    },
    ModelParamSpec {
        name: "vdsat0",
        default: Some("0"),
        description: "Nominal drain saturation voltage of the impact ionization current",
    },
    ModelParamSpec {
        name: "hiit",
        default: Some("0"),
        description: "Temperature dependent parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiil",
        default: Some("0"),
        description: "Channel length dependent parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiie",
        default: Some("0"),
        description: "Saturation channel electric field for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivg0",
        default: Some("0"),
        description: "First Vgs dependent parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivg1",
        default: Some("0"),
        description: "Second Vgs dependent parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivg2",
        default: Some("0"),
        description: "Third Vgs dependent parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivgd",
        default: Some("0"),
        description: "Vds dependent parameter for the impact ionization current",
    },
    ModelParamSpec {
        name: "hiivoff",
        default: Some("-1000"),
        description: "Offset voltage for Vgstep limiting (-1000 disables it)",
    },
    ModelParamSpec {
        name: "isubmode",
        default: Some("1"),
        description: "Impact ionization current vs. substrate current model flag",
    },
    ModelParamSpec {
        name: "dlhci",
        default: Some("0"),
        description: "Length dependence offset for the HCI model",
    },
    ModelParamSpec {
        name: "dwhci",
        default: Some("0"),
        description: "Width dependence offset for the HCI model",
    },
    ModelParamSpec {
        name: "vbsmax",
        default: Some("1e6"),
        description: "Positive Vbs limiting value for the HCI model",
    },
];
