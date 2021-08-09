#include <iostream>
#include <exception>
#include <string>
#include <unistd.h>

#include "citeproc_rs.hpp"

using namespace citeproc_rs;

const std::string style = "<style xmlns=\"http://purl.org/net/xbiblio/csl\" class=\"note\" version=\"1.0\" default-locale=\"en-GB\">"
               "<info><id>id</id><title>title</title><updated>2015-10-10T23:31:02+00:00</updated></info>"
               "<citation><layout delimiter=\"; \"><group delimiter=\", \"><names variable=\"author\" /><date variable=\"issued\" form=\"numeric\" /></group></layout></citation>"
               "<bibliography><layout><group delimiter=\", \">"
               "<names variable=\"author\" />"
               "<text variable=\"title\" font-style=\"italic\" />"
               "</group></layout></bibliography>"
               "</style>";

const std::string en_us = "<locale version=\"1.0\" xml:lang=\"en-US\">\n"
                "<info> <updated>2015-10-10T23:31:02+00:00</updated> </info>"
                "<terms> </terms>"
                "</locale>";

void locale_fetch_callback(void *context, LocaleSlot *slot, const char *lang) {
        const char *ctx_str = *((char **)context);
        std::cout << "context carried: " << ctx_str << std::endl;
        citeproc_rs_locale_slot_write(slot, en_us.data(), en_us.length());
}

void buffer_write(void *user_data, const uint8_t *src, size_t src_len) {
        std::string *string = (std::string *) user_data;
        const char *as_char = (const char *)src;
        string->append(as_char, src_len);
}

void buffer_clear(void *user_data) {
        std::string *string = (std::string *) user_data;
        string->clear();
}

const BufferOps std_buffer_ops = BufferOps { 
    .write = buffer_write,
    .clear = buffer_clear,
};

void log_write(void *user_data, LogLevel level, const uint8_t *modpath, size_t modpath_len, const uint8_t *message, size_t message_len) {
    printf("[%.*s] %.*s\n", (int) modpath_len, modpath, (int) message_len, message);
}

const FFILoggerVTable logger_ops = FFILoggerVTable {
    .write = log_write,
    .flush = NULL,
};

int main() {
        std::string err;

        const std::string filters = "citeproc_proc::db=info";
        if (citeproc_rs_set_logger((void *)NULL, logger_ops, LevelFilter::warn, filters.data(), filters.length()) != ErrorCode::none) {
            citeproc_rs_last_error_utf8(std_buffer_ops, &err);
            printf("failed to set logger: %s\n", err.data());
            exit(1);
        }

        const char *context_ex = "example context";
        void *context = (void *) &context_ex;
        InitOptions init = {
                .style = style.data(),
                .style_len = style.length(),
                .locale_fetch_context = context,
                .locale_fetch_callback = locale_fetch_callback,
                .format = OutputFormat::html,
                .buffer_ops = std_buffer_ops,
        };
        Driver *proc = citeproc_rs_driver_new(init);
        if (proc == NULL) {
            citeproc_rs_last_error_utf8(std_buffer_ops, &err);
            printf("failed to init driver: %s\n", err.data());
            exit(1);
        }

        std::string rendered;

        std::string ref_json = "{"
                "\"id\": \"item\","
                "\"type\": \"book\","
                "\"title\": \"the title\""
        "}";
        ErrorCode code = citeproc_rs_driver_preview_reference(proc, ref_json.data(), ref_json.length(), OutputFormat::html, &rendered);
        if (code == ErrorCode::none) {
                printf("%s\n", rendered.data());
                // assert(rendered.compare("the title") == 0);
                std::cout << "success: " << rendered << std::endl;
        } else {
                citeproc_rs_last_error_utf8(std_buffer_ops, &err);
                std::cout << err << std::endl;
        }
        // this is allocated via rust Box and needs to be deallocated using
        // Box::from_raw so just pass it back, the library knows what to do.
        citeproc_rs_driver_free(proc);
}
