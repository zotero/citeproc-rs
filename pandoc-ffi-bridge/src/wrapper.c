#include <HsFFI.h>

void
panbridge_init (void)
{
  hs_init (0, 0);
}

void
panbridge_exit (void)
{
  hs_exit ();
}
