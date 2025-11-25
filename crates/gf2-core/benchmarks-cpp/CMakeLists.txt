cmake_minimum_required(VERSION 3.10)
project(gf2_cpp_benchmarks CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -O3 -march=native")

# Find libraries
find_library(NTL_LIB ntl REQUIRED)
find_library(M4RI_LIB m4ri REQUIRED)
find_library(FLINT_LIB flint REQUIRED)

include_directories(/usr/include)

# NTL benchmarks
add_executable(bench_ntl_field bench_ntl_field.cpp)
target_link_libraries(bench_ntl_field ${NTL_LIB} gmp pthread)

# M4RI benchmarks
add_executable(bench_m4ri_matrix bench_m4ri_matrix.cpp)
target_link_libraries(bench_m4ri_matrix ${M4RI_LIB} m)

add_executable(bench_m4ri_gauss bench_m4ri_gauss.cpp)
target_link_libraries(bench_m4ri_gauss ${M4RI_LIB} m)

# FLINT benchmarks
add_executable(bench_flint_poly bench_flint_poly.cpp)
target_link_libraries(bench_flint_poly ${FLINT_LIB} gmp mpfr)

message(STATUS "NTL: ${NTL_LIB}")
message(STATUS "M4RI: ${M4RI_LIB}")
message(STATUS "FLINT: ${FLINT_LIB}")
