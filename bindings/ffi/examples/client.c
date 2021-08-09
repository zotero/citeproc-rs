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
               "<citation><layout delimiter=\"; \"><group delimiter=\", \"><names variable=\"author\" /><date variable=\"issued\" form=\"numeric\" /></group></layout></citation>"
               "<bibliography><layout><group delimiter=\", \">"
               "<names variable=\"author\" />"
               "<text variable=\"title\" font-style=\"italic\" />"
               "</group></layout></bibliography>"
               "</style>");

LIT_LEN(en_us, "<locale version=\"1.0\" xml:lang=\"en-US\">\n"
                "<info> <updated>2015-10-10T23:31:02+00:00</updated> </info>"
                "<terms> </terms>"
                "</locale>");

void locale_fetch_callback(void *context, citeproc_rs_locale_slot *slot, const char *lang) {
        printf("context carried: %s\n", *((char **)context));
        citeproc_rs_locale_slot_write(slot, en_us, en_us_len);
}

const citeproc_rs_buffer_ops buffer_ops = citeproc_rs_cstring_buffer_ops;

void log_write(void *user_data, citeproc_rs_log_level level, const uint8_t *modpath, size_t modpath_len, const uint8_t *message, size_t message_len) {
    printf("[%.*s] %.*s\n", (int) modpath_len, modpath, (int) message_len, message);
}

const citeproc_rs_ffi_logger_v_table logger_ops = (citeproc_rs_ffi_logger_v_table) {
    .write = log_write,
    .flush = NULL,
};

// provided by citeproc-ffi
void test_log_msg(size_t l, const char *a, size_t len);

int main() {

        LIT_LEN(log_filter, "debug");
        citeproc_rs_set_logger(NULL, logger_ops, CITEPROC_RS_LEVEL_FILTER_WARN, log_filter, log_filter_len);

        test_log_msg(1, "hi", 2);

        char *context_ex = "example context";
        void *context = (void *) &context_ex;
        char *rendered = NULL;

        char *err = NULL;
        citeproc_rs_error_code code;
#define handle_error(code) if (code) { \
        citeproc_rs_last_error_utf8(buffer_ops, &err); \
        printf("error (%s line %d): %s\n", __FILE__, __LINE__, err); \
        return 1; \
}


        citeproc_rs_init_options init = {
                .style = style,
                .style_len = style_len,
                .locale_fetch_context = context,
                .locale_fetch_callback = locale_fetch_callback,
                .format = CITEPROC_RS_OUTPUT_FORMAT_HTML,
                .buffer_ops = buffer_ops,
        };
        citeproc_rs_driver *driver = citeproc_rs_driver_new(init);
        if (!driver) {
                citeproc_rs_last_error_utf8(buffer_ops, &err);
                printf("err creating driver: %s\n", err);
                return 1;
        }

        const char *ref_json = "{"
                "\"id\": \"item\","
                "\"type\": \"book\","
                "\"issued\": { \"raw\": \"1951\" },"
                "\"title\": \"The Origins of Totalitarianism\","
                "\"author\": [{ \"given\": \"Hannah\", \"family\": \"Arendt\" }]"
        "}";
        size_t ref_json_len = strlen(ref_json);

        handle_error(citeproc_rs_driver_preview_reference(
                                driver, ref_json, ref_json_len,
                                CITEPROC_RS_OUTPUT_FORMAT_HTML, &rendered));
        printf("previewed reference: %s\n", rendered);
        assert(strcmp(rendered, "Hannah Arendt, <i>The Origins of Totalitarianism</i>") == 0);

        // we're happy with that, but previewing doesn't save it.
        // so we'll insert the reference properly:
        handle_error(citeproc_rs_driver_insert_reference(driver, ref_json, ref_json_len));

        citeproc_rs_cluster_id id = 1;
        citeproc_rs_cluster *cluster = citeproc_rs_cluster_new(id);

        // we'll make two of the same
        // technically these return values can all be negative error codes too
        // but let's just ignore that here
        LIT_LEN(ref_id, "item");
        uint32_t cite_1 = (uint32_t) citeproc_rs_cluster_cite_new(cluster, ref_id, ref_id_len);
        uint32_t cite_2 = (uint32_t) citeproc_rs_cluster_cite_new(cluster, ref_id, ref_id_len);

        // configure the first cite
        LIT_LEN(prefix, "prefix: ");
        handle_error(citeproc_rs_cluster_cite_set_prefix(cluster, cite_1, prefix, prefix_len));

        handle_error(citeproc_rs_driver_insert_cluster(driver, cluster));

        citeproc_rs_cluster_position *positions = malloc(1 * sizeof(citeproc_rs_cluster_position));
        positions[0] = (citeproc_rs_cluster_position) {
                .id = id,
                .is_preview_marker = false,
                .is_note = true,
                .note_number = 1,
        };

        handle_error(citeproc_rs_driver_set_cluster_order(driver, positions, 1));
        free(positions);

        handle_error(citeproc_rs_driver_format_cluster(driver, id, &rendered));
        printf("cluster %d: %s\n", id, rendered);

        handle_error(citeproc_rs_driver_format_bibliography(driver, &rendered));
        printf("bibliography: \n%s\n", rendered);

        // we allocated these with cstring in the buffer_write_callback
        // if not though, calling free on NULL is fine
        citeproc_rs_cstring_free(rendered);
        citeproc_rs_cstring_free(err);

        // but this one is allocated via rust Box and needs to be deallocated using Box::from_raw
        // so just pass it back, the library knows what to do.
        citeproc_rs_driver_free(driver);
}
