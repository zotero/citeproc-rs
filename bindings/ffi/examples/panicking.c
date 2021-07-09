#include "citeproc_rs.h"
#include <stdlib.h>
#include <stdio.h>
#include <assert.h>

int main() {
        citeproc_rs_log_init();
        struct citeproc_rs_cool_struct coolio = { .field = 5 };
        if (!viva_la_funcion(&coolio, 100)) {
                uintptr_t msg_len = citeproc_rs_last_error_length();
                char *buf = malloc(msg_len);
                intptr_t bytes_written = citeproc_rs_error_message_utf8(buf, msg_len);
                if (bytes_written > 0) {
                        printf("error occurred: %s\n", buf);
                }
                assert(coolio.field == 0);
        }
}
