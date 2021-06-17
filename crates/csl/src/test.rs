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
fn intext() {
    let features = Features {
        custom_intext: true,
        ..Default::default()
    };
    let options = ParseOptions {
        allow_no_info: true,
        features: Some(features),
        ..Default::default()
    };
    assert_snapshot_parse!(
        InText,
        r#"<intext><layout><text variable="title"/></layout></intext>"#
    );
    assert_snapshot_parse!(
        Style,
        r#"<style class="in-text">
            <citation><layout></layout></citation>
            <intext><layout><text variable="title" /></layout></intext>
        </style>"#,
        options.clone()
    );
    assert_snapshot_err!(
        Style,
        r#"<style class="in-text">
             <citation><layout></layout></citation>
             <intext><layout></layout></intext>
         </style>"#
    );
}

#[test]
fn unsupported_version() {
    assert_snapshot_err!(
        Style,
        r#"
        <style version="999.0" class="in-text">
            <citation><layout></layout></citation>
        </style>
    "#
    );
}

#[test]
fn unrecognised_macros() {
    assert_snapshot_err!(
        Style,
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
    assert_snapshot_err!(
        Style,
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
    assert_snapshot_err!(
        Style,
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
    assert_snapshot_parse!(
        Style,
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
    assert_snapshot_parse!(
        Style,
        r#"
        <style version="1.0.1" class="in-text">
            <citation><layout></layout></citation>
        </style>
    "#
    );
}

#[test]
fn wrong_tag_name() {
    assert_snapshot_err!(
        Style,
        r#"
        <stylo version="1.0.1" class="in-text">
            <citation><layout></layout></citation>
        </stylo>
    "#
    );
    assert_snapshot_err!(
        Locale,
        r#"
        <localzzz xml:lang="en-US" version="1.0.1" class="in-text">
        </localzzz>
    "#
    );
}
