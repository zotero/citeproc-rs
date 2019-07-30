// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship


/// [Spec](https://docs.citationstyles.org/en/stable/specification.html#appendix-i-categories)
#[derive(AsRefStr, EnumProperty, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Categories {
    Anthropology,
    Astronomy,
    Biology,
    Botany,
    Chemistry,
    Communications,
    Engineering,
    GenericBase, // UsedForGenericStylesLikeHarvardAndApa
    Geography,
    Geology,
    History,
    Humanities,
    Law,
    Linguistics,
    Literature,
    Math,
    Medicine,
    Philosophy,
    Physics,
    #[strum(serialize="political_science")]
    PoliticalScience,
    Psychology,
    Science,
    #[strum(serialize="social_science")]
    SocialScience,
    Sociology,
    Theology,
    Zoology,
}


