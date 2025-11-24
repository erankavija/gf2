# DVB T2 Test Vectors

This document presents an overview of the available test vectors for DVB T2 for FEC and interleaving. These test vectors are essential for validating implementations of the DVB T2 standard. The following test points are included:

TestPoint4 -> BCH encoding -> TestPoint5 -> LDPC encoding -> TestPoint6 -> Bit Interleaving -> TestPoint7a

The test data is organized into text files by test point, with each file containing the data for a specific test point. For all the test points 4-7a, the data is represented in binary string format split into lines of 64 bits for readability.

## Test Vector Files

Each archive file shall contain a subdirectory for each test point, having the name TestPointXX, where XX
is the number of the test point in zero-padded two-digit decimal format. Each test-point directory will contain
stream files from different companies. The files will obey the following naming convention:

<VVReference>_TP<xx>_<company>.txt

where VVReference is the configuration name and <xx> is the number of the test point, as defined
in section 2 and zero-padded to two digits. As above, <company> indicates the implementation that
generated the files and for the published streams is always CSP.
For test points that have an ‘a’ or ‘b’ suffix, these shall be included in the relevant base test point directory,
i.e. <VVReference>_TP08a_<company>.txt shall reside in the directory TestPoint08.
