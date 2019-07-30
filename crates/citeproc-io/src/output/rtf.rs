// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::{LocalizedQuotes, OutputFormat};
use crate::utils::JoinMany;
use csl::style::{
    FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment,
};

use rtf_grimoire::tokenizer::Token;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Rtf;
