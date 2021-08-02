#include <stdlib.h>
#include <stdio.h>
#include <assert.h>
#include <string.h>

#include "citeproc_rs.h"

// length excluding null terminator
#define STRLEN(s) (sizeof(s)/sizeof(s[0]) - 1)
#define LIT_LEN(name, lit) const char *name = (lit); uintptr_t name##_len = STRLEN(lit)

LIT_LEN(style, "<style xmlns=\"http://purl.org/net/xbiblio/csl\" class=\"note\" version=\"1.0\" default-locale=\"en-GB\">"
               "<info><id>id</id><title>title</title><updated>2015-10-10T23:31:02+00:00</updated></info>"
               "<citation><layout><text variable=\"title\" /></layout></citation></style>");

LIT_LEN(en_us, "<locale version=\"1.0\" xml:lang=\"en-US\">\n"
                "<info> <updated>2015-10-10T23:31:02+00:00</updated> </info>"
                "<terms> </terms>"
                "</locale>");

void locale_fetch_callback(void *context, citeproc_rs_locale_slot *slot, const char *lang) {
        printf("context carried: %s\n", *((char **)context));
        citeproc_rs_locale_slot_write(slot, en_us, en_us_len);
}

const citeproc_rs_buffer_ops buffer_ops = citeproc_rs_managed_buffer_ops;

int main() {
        char *context_ex = "example context";
        void *context = (void *) &context_ex;
        citeproc_rs_init_options init = {
                .style = style,
                .style_len = style_len,
                .locale_fetch_context = context,
                .locale_fetch_callback = locale_fetch_callback,
                .format = CITEPROC_RS_OUTPUT_FORMAT_HTML,
                .buffer_ops = buffer_ops,
        };
        citeproc_rs_driver *proc = citeproc_rs_driver_new(init);

        const char *ref_json = "{"
                "\"id\": \"item\","
                "\"type\": \"book\","
                "\"title\": \"the title\""
        "}";
        size_t ref_json_len = strlen(ref_json);
        char *rendered = NULL;
        char *err = NULL;

        citeproc_rs_error_code code;

        code = citeproc_rs_driver_preview_reference(proc, ref_json, ref_json_len, &rendered);
        if (code == CITEPROC_RS_ERROR_CODE_NONE) {
                assert(strcmp(rendered, "the title") == 0);
                printf("success: %s\n", rendered);
        } else {
                citeproc_rs_last_error_utf8(buffer_ops, &err);
                printf("err: %s", err);
        }
        // we allocated these two with managed in the buffer_write_callback
        // calling free on NULL is fine
        citeproc_rs_string_free(rendered);
        citeproc_rs_string_free(err);
        // but this one is allocated via rust Box and needs to be deallocated using Box::from_raw
        // so just pass it back, the library knows what to do.
        citeproc_rs_driver_free(proc);
}
