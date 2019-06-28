// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

#[derive(Serialize, Deserialize, Debug)]
pub enum OutputNode {
    Fmt(FormatNode),
    Str(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FormatNode {
    formatting: Formatting,
    children: Vec<OutputNode>,
}


