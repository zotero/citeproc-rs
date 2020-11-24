// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

use super::*;

#[test]
fn features() {
    assert_snapshot_parse!(Features, r#"<features></features>"#);
    assert_snapshot_parse!(
        Features,
        r#"
        <features>
            <feature name="condition-date-parts" />
            <feature name="edtf-dates" />
        </features>
    "#
    );
    assert_snapshot_err!(
        Features,
        r#"
        <features>
            <feature name="edtf-dates" />
            <feature name="UNRECOGNIZED-FEATURE" />
            <feature name="SECOND-UNRECOGNIZED-FEATURE" />
        </features>
    "#
    );
}

#[test]
fn unsupported_version() {
    assert_snapshot_style_err!(
        r#"
        <style version="999.0" class="in-text">
            <citation><layout></layout></citation>
        </style>
    "#
    );
}

#[test]
fn unrecognised_macros() {
    assert_snapshot_style_err!(
        r#"
        <style version="1.0" class="in-text">
            <citation>
                <layout>
                    <text macro="unknown" />
                </layout>
            </citation>
        </style>
    "#
    );
    assert_snapshot_style_err!(
        r#"
        <style version="1.0" class="in-text">
            <citation>
                <sort>
                    <key macro="unknown" />
                </sort>
                <layout></layout>
            </citation>
        </style>
    "#
    );
    assert_snapshot_style_err!(
        r#"
        <style version="1.0" class="in-text">
            <citation><layout></layout></citation>
            <bibliography>
                <sort>
                    <key macro="unknown" />
                </sort>
                <layout></layout>
            </bibliography>
        </style>
    "#
    );
    assert_snapshot_style_parse!(
        r#"
        <style version="1.0" class="in-text">
            <macro name="known" />
            <citation>
                <layout>
                    <text macro="known" />
                </layout>
            </citation>
        </style>
    "#
    );
}

#[test]
fn missing_info() {
    // Externally, missing info should fail.
    insta::assert_debug_snapshot!(crate::Style::parse(::indoc::indoc!(
        r#"
            <style version="1.0.1" class="in-text">
                <citation><layout></layout></citation>
            </style>
        "#
    ))
    .expect_err("should have failed with errors"));
    // But internally we can ignore it.
    assert_snapshot_style_parse!(
        r#"
        <style version="1.0.1" class="in-text">
            <citation><layout></layout></citation>
        </style>
    "#
    );
}
