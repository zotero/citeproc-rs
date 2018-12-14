
#define CAT(a,b) XCAT(a,b)
#define XCAT(a,b) a ## b
#define STR(a) XSTR(a)
#define XSTR(a) #a
 
#include <HsFFI.h>
 
extern void CAT(__stginit_, Lib)(void);
 
void panbridge_init(void) __attribute__((constructor));
void panbridge_init(void)
{
    /* This seems to be a no-op, but it makes the GHCRTS envvar work. */
    static char *argv[] = { STR(Lib) ".so", 0 }, **argv_ = argv;
    static int argc = 1;
 
    hs_init(&argc, &argv_);
    /* hs_add_root(CAT(__stginit_, Lib)); */
}
 
void panbridge_exit(void) __attribute__((destructor));
void panbridge_exit(void)
{
    hs_exit();
}
