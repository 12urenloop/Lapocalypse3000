#include <Dw3000/src/dw3000.h>

inline void print(char* s){
    UART_puts(s);
}

inline void println(char* s){
    print(s);
    print("\n");
}