#include <iostream>
#include <exception>
#include <string>

#include "citeproc_rs.hpp"

using namespace citeproc_rs;

const std::string style = "<style xmlns=\"http://purl.org/net/xbiblio/csl\" class=\"note\" version=\"1.0\" default-locale=\"en-GB\">"
               "<info><id>id</id><title>title</title><updated>2015-10-10T23:31:02+00:00</updated></info>"
               "<citation><layout><text variable=\"title\" /></layout></citation></style>";

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

int main() {
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

        std::string ref_json = "{"
                "\"id\": \"item\","
                "\"type\": \"book\","
                "\"title\": \"the title\""
        "}";
        std::string rendered;
        std::string err;
        ErrorCode code = citeproc_rs_driver_preview_reference(proc, ref_json.data(), ref_json.length(), &rendered);
        if (code == ErrorCode::none) {
                assert(rendered.compare("the title") == 0);
                std::cout << "success: " << rendered << std::endl;
        } else {
                citeproc_rs_last_error_utf8(std_buffer_ops, &err);
                std::cout << err << std::endl;
        }
        // this is allocated via rust Box and needs to be deallocated using
        // Box::from_raw so just pass it back, the library knows what to do.
        citeproc_rs_driver_free(proc);
}
