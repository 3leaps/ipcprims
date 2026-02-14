#include <stdint.h>
#include <stdlib.h>

#include "ipcprims.h"

int main(void) {
  if (ipc_init() != IPC_RESULT_OK) {
    return 1;
  }

  const char *err = ipc_last_error();
  if (err == NULL) {
    return 2;
  }

  IpcFrame frame = {0};
  ipc_frame_free(&frame);

  ipc_cleanup();
  return 0;
}
