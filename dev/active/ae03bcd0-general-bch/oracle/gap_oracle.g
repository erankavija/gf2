#############################################################################
##
##  GAP/GUAVA oracle for the predeclared BCH conformance corpus.
##
##  Reads the corpus emitted by the `bch_oracle_messages` example, rebuilds
##  each row inside GAP with GUAVA, and writes the generator polynomials,
##  dimensions, defining sets and codewords GUAVA derives.
##
##  Two witnesses per row
##  ---------------------
##  A BCH code is fixed by (q, n, b, delta) together with the n-th root of
##  unity it is built on, so each row carries two GUAVA results.
##
##    * `bchcode_*`: GUAVA's unaided `BCHCode(n, b, delta, F)`, built on
##      GUAVA's own `PrimitiveUnityRoot(q, n)`. The root is written back in
##      gf2's coordinates as `guava_root_gf2_index`, so gf2 can construct the
##      same code from an explicitly supplied root and compare.
##    * `generator`, `codewords_*`: the same GUAVA derivation run at gf2's own
##      alpha, which is `PrimitiveUnityRoot` replaced by the transported root
##      in the cyclotomic-coset loop over `MinimalPolynomial`, wrapped by
##      `GeneratorPolCode`.
##
##  Where the two roots coincide the two witnesses are the same code, which
##  `bchcode_generator_matches` records.
##
##  Run it as
##      gap -q -A -o 4g -c 'CORPUS:="<corpus.json>"; OUTPUT:="<output.json>";' \
##          gap_oracle.g
##
##  Field transport
##  ---------------
##  GAP presents every GF(p^d) by its Conway polynomial and offers no way to
##  substitute another defining polynomial, so a row whose gf2 presentation is
##  not the Conway one is carried across by an explicit isomorphism: gf2's base
##  generator y maps to a root of gf2's base modulus in GAP's GF(q), and gf2's
##  splitting-field generator z maps to a root of the image of gf2's splitting
##  modulus in GAP's GF(q^s). Both root choices are fixed as the root of least
##  canonical index and recorded. A different choice differs by an element of
##  the relevant Galois group, which fixes the base field pointwise, so the
##  generator polynomial is the same either way.
##
##  Canonical index
##  ---------------
##  A field element is written as the integer whose base-p digits are its
##  coordinates in a named basis, coordinate zero least significant.
##
##  Generator coefficients and codeword symbols are base-field elements and are
##  written in *gf2's* coordinates, that is in the basis of powers of the image
##  of gf2's base generator, so they compare directly with the corpus. The
##  isomorphism data -- `alpha_index`, `base_generator_image` and
##  `splitting_generator_image` -- are written in GAP's own canonical basis
##  1, Z(p^d), ..., Z(p^d)^(d-1), which is the polynomial basis of the Conway
##  root, because they describe GAP's presentation.
##
##  A splitting-field element is written in gf2's coordinates -- the fields
##  `alpha_gf2_index` and `guava_root_gf2_index` -- through the inverse of the
##  same isomorphism: the GF(p)-basis of products (base generator image)^j
##  (splitting generator image)^i carries digit j + dB*i, which is exactly the
##  grouping the corpus's canonical index uses. `alpha_gf2_index` reproduces
##  the corpus's own `alpha`, which is the round trip that validates the
##  inverse map before `guava_root_gf2_index` is read out of it.
##
##  Scale limit
##  -----------
##  GUAVA's `BCHCode` reaches `GeneratorPolCode`, which materializes the whole
##  generator matrix through `GeneratorMatrixFromPoly`. At the DVB-T2 mother
##  length that matrix is 65343 x 65535 and exhausts memory before any code
##  object exists. That row therefore runs GUAVA's `BCHCode` generator
##  derivation -- `PrimitiveUnityRoot` replaced by gf2's root, then the same
##  cyclotomic-coset loop over `MinimalPolynomial` -- without the code-object
##  wrapper, and the fixture records that. Every other row builds the wrapper
##  too and checks that its generator is the one the loop derived.
##
##  `ORACLE_GAP_HEAP` in `run.sh` sets the GAP heap this script runs under and
##  the receipt records the value used.
##

LoadPackage("guava");;

HEX := "0123456789abcdef";;
NATIVE_CODE_LIMIT := 4096;;

#############################################################################
##  A minimal reader for the corpus subset of JSON: objects, arrays,
##  unescaped strings, non-negative integers, booleans and null.

JsonSkip := function(st)
    while st.pos <= Length(st.text) and st.text[st.pos] in " \t\n\r" do
        st.pos := st.pos + 1;
    od;
end;;

JsonString := function(st)
    local start;
    st.pos := st.pos + 1;
    start := st.pos;
    while st.text[st.pos] <> '"' do st.pos := st.pos + 1; od;
    st.pos := st.pos + 1;
    return st.text{[start .. st.pos - 2]};
end;;

JsonNumber := function(st)
    local start;
    start := st.pos;
    while st.pos <= Length(st.text) and st.text[st.pos] in "-0123456789" do
        st.pos := st.pos + 1;
    od;
    return Int(st.text{[start .. st.pos - 1]});
end;;

JsonObject := 0;;
JsonArray := 0;;

JsonValue := function(st)
    local c;
    JsonSkip(st);
    c := st.text[st.pos];
    if c = '{' then return JsonObject(st);
    elif c = '[' then return JsonArray(st);
    elif c = '"' then return JsonString(st);
    elif c = 't' then st.pos := st.pos + 4; return true;
    elif c = 'f' then st.pos := st.pos + 5; return false;
    elif c = 'n' then st.pos := st.pos + 4; return fail;
    else return JsonNumber(st);
    fi;
end;;

JsonArray := function(st)
    local out;
    out := [];
    st.pos := st.pos + 1;
    JsonSkip(st);
    if st.text[st.pos] = ']' then st.pos := st.pos + 1; return out; fi;
    repeat
        Add(out, JsonValue(st));
        JsonSkip(st);
        if st.text[st.pos] = ',' then st.pos := st.pos + 1; else break; fi;
    until false;
    st.pos := st.pos + 1;
    return out;
end;;

JsonObject := function(st)
    local out, key;
    out := rec();
    st.pos := st.pos + 1;
    JsonSkip(st);
    if st.text[st.pos] = '}' then st.pos := st.pos + 1; return out; fi;
    repeat
        JsonSkip(st);
        key := JsonString(st);
        JsonSkip(st);
        st.pos := st.pos + 1;
        out.(key) := JsonValue(st);
        JsonSkip(st);
        if st.text[st.pos] = ',' then st.pos := st.pos + 1; else break; fi;
    until false;
    st.pos := st.pos + 1;
    return out;
end;;

JsonParse := function(text)
    return JsonValue(rec(text := text, pos := 1));
end;;

#############################################################################
##  Canonical indices and the corpus hex convention.

IndexOfFFE := function(e, p, d)
    local coefficients, index, i;
    if d = 1 then return IntFFE(e); fi;
    coefficients := Coefficients(CanonicalBasis(GF(p ^ d)), e);
    index := 0;
    for i in [d, d - 1 .. 1] do
        index := index * p + IntFFE(coefficients[i]);
    od;
    return index;
end;;

##  Returns the canonical index of a base-field element in *gf2's*
##  coordinates: its coordinates in the basis of powers of the image of gf2's
##  base generator, which is the isomorphism this script pins.
BaseIndexOfFFE := function(e, p, dB, basis)
    local coefficients, index, i;
    if dB = 1 then return IntFFE(e); fi;
    coefficients := Coefficients(basis, e);
    index := 0;
    for i in [dB, dB - 1 .. 1] do
        index := index * p + IntFFE(coefficients[i]);
    od;
    return index;
end;;

##  Returns the canonical index of a splitting-field element in *gf2's*
##  coordinates: its coordinates in the GF(p)-basis of products of powers of
##  the images of gf2's base and splitting generators, digit j + dB*i carrying
##  (base image)^j (splitting image)^i. This inverts the transport that carries
##  gf2's alpha into GAP's field.
Gf2IndexOfFFE := function(e, p, dE, basis)
    local coefficients, index, i;
    if dE = 1 then return IntFFE(e); fi;
    coefficients := Coefficients(basis, e);
    index := 0;
    for i in [dE, dE - 1 .. 1] do
        index := index * p + IntFFE(coefficients[i]);
    od;
    return index;
end;;

##  Reads an index as an element of GF(p^d), placing digit i on `gen`^i.
FFEFromIndex := function(index, p, d, gen)
    local element, power, remaining, i;
    if d = 1 then return index * One(GF(p)); fi;
    element := Zero(GF(p ^ d));
    power := One(GF(p ^ d));
    remaining := index;
    for i in [1 .. d] do
        element := element + (remaining mod p) * power;
        remaining := QuoInt(remaining, p);
        power := power * gen;
    od;
    return element;
end;;

DecodeSymbols := function(text, count, baseOrder)
    local bytes, i, high, low, out;
    bytes := [];
    for i in [1 .. QuoInt(Length(text), 2)] do
        high := Position(HEX, text[2 * i - 1]) - 1;
        low := Position(HEX, text[2 * i]) - 1;
        Add(bytes, 16 * high + low);
    od;
    out := [];
    if baseOrder = 2 then
        for i in [0 .. count - 1] do
            Add(out, QuoInt(bytes[QuoInt(i, 8) + 1], 2 ^ (i mod 8)) mod 2);
        od;
    else
        for i in [1 .. count] do Add(out, bytes[i]); od;
    fi;
    return out;
end;;

EncodeSymbols := function(symbols, baseOrder)
    local bytes, i, text, byte;
    bytes := [];
    if baseOrder = 2 then
        for i in [1 .. QuoInt(Length(symbols) + 7, 8)] do Add(bytes, 0); od;
        for i in [0 .. Length(symbols) - 1] do
            if symbols[i + 1] = 1 then
                bytes[QuoInt(i, 8) + 1] := bytes[QuoInt(i, 8) + 1] + 2 ^ (i mod 8);
            fi;
        od;
    else
        bytes := ShallowCopy(symbols);
    fi;
    text := "";
    for byte in bytes do
        Add(text, HEX[QuoInt(byte, 16) + 1]);
        Add(text, HEX[byte mod 16 + 1]);
    od;
    return text;
end;;

#############################################################################
##  JSON emission.

JsonQuote := function(text) return Concatenation("\"", text, "\""); end;;

JsonInts := function(values)
    local out, i;
    out := "[";
    for i in [1 .. Length(values)] do
        if i > 1 then Append(out, ", "); fi;
        Append(out, String(values[i]));
    od;
    Append(out, "]");
    return out;
end;;

JsonStrings := function(values)
    local out, i;
    out := "[";
    for i in [1 .. Length(values)] do
        if i > 1 then Append(out, ", "); fi;
        Append(out, JsonQuote(values[i]));
    od;
    Append(out, "]");
    return out;
end;;

JsonBool := function(value) if value then return "true"; else return "false"; fi; end;;

JsonMaybeBool := function(value)
    if value = fail then return "null"; fi;
    return JsonBool(value);
end;;

JsonMaybeInt := function(value)
    if value = fail then return "null"; fi;
    return String(value);
end;;

#############################################################################
##  Finds the root of least canonical index of `poly` in GF(p^d).

LeastRoot := function(poly, p, d)
    local best, bestIndex, element, index;
    best := fail;
    bestIndex := fail;
    for element in GF(p ^ d) do
        if IsZero(Value(poly, element)) then
            index := IndexOfFFE(element, p, d);
            if bestIndex = fail or index < bestIndex then
                best := element;
                bestIndex := index;
            fi;
        fi;
    od;
    return best;
end;;

#############################################################################
##  Rebuilds one corpus row and returns its oracle record as JSON text.

BuildRow := function(row)
    local p, dB, dE, r, q, n, delta, b0, F, Fs, x, conwayBase, baseGen,
          basePresentation, extGen, extPresentation, conwayExt, modulus,
          poly, alpha, pur, G, powerSet, test, t, coset, cosets, definingSet,
          j, k, code, nativeCode, bchGenerator, bchMatches, unavailable,
          messages, native, systematic, symbols, message, product, shifted,
          remainder, systematicPoly, coefficients, out, i, encoderNote,
          crossChecked, checkWord, divides, rootsChecked, baseBasis,
          messageFFE, gf2Basis, alphaGf2Index, guavaRootGf2Index, bchPoly,
          bchK, bchDefiningSet, bchNative, bchSystematic;

    p := row.characteristic;
    dB := row.base_degree;
    dE := row.ext_degree;
    r := row.relative_degree;
    q := row.base_order;
    n := row.n;
    delta := row.designed_distance;
    b0 := row.first_root;
    F := GF(q);
    Fs := GF(p ^ dE);
    x := Indeterminate(F, 1);

    #  The image of gf2's base generator y.
    if dB = 1 then
        baseGen := One(F);
        basePresentation := "prime base field";
    else
        conwayBase := UnivariatePolynomial(GF(p),
            List(row.base_modulus, c -> c * One(GF(p))), 1);
        if CoefficientsOfUnivariatePolynomial(ConwayPolynomial(p, dB))
           = CoefficientsOfUnivariatePolynomial(conwayBase) then
            baseGen := Z(q);
            basePresentation := "conway";
        else
            baseGen := LeastRoot(conwayBase, p, dB);
            basePresentation := "transported";
        fi;
    fi;

    baseBasis := Basis(F, List([0 .. dB - 1], i -> baseGen ^ i));

    #  The image of gf2's splitting-field generator z.
    if r = 1 then
        extGen := One(Fs);
        extPresentation := "splitting field equals base field";
    elif dB = 1 then
        conwayExt := UnivariatePolynomial(GF(p),
            List(row.ext_modulus, c -> c * One(GF(p))), 1);
        if CoefficientsOfUnivariatePolynomial(ConwayPolynomial(p, dE))
           = CoefficientsOfUnivariatePolynomial(conwayExt) then
            extGen := Z(p ^ dE);
            extPresentation := "conway";
        else
            extGen := LeastRoot(conwayExt, p, dE);
            extPresentation := "transported";
        fi;
    else
        modulus := UnivariatePolynomial(Fs,
            List(row.ext_modulus, c -> FFEFromIndex(c, p, dB, baseGen)), 1);
        extGen := LeastRoot(modulus, p, dE);
        extPresentation := "transported";
    fi;
    if extGen = fail then
        Error("gf2's splitting-field modulus has no root in GAP's field");
    fi;

    #  The GF(p)-basis in which a splitting-field element reads as gf2's own
    #  canonical index, digit j + dB*i on (base image)^j (splitting image)^i.
    gf2Basis := Basis(Fs, List([0 .. dE - 1],
        m -> baseGen ^ (m mod dB) * extGen ^ QuoInt(m, dB)));

    #  gf2's alpha, carried across by the composite isomorphism.
    alpha := Zero(Fs);
    t := row.alpha;
    poly := One(Fs);
    for i in [1 .. r] do
        alpha := alpha + FFEFromIndex(t mod q, p, dB, baseGen) * poly;
        t := QuoInt(t, q);
        poly := poly * extGen;
    od;
    if Order(alpha) <> n then
        Error("the transported root does not have order n");
    fi;

    #  The inverse map, validated on the element the forward map just built.
    alphaGf2Index := Gf2IndexOfFFE(alpha, p, dE, gf2Basis);
    if alphaGf2Index <> row.alpha then
        Error("the inverse transport does not return gf2's alpha");
    fi;

    pur := PrimitiveUnityRoot(q, n);
    guavaRootGf2Index := Gf2IndexOfFFE(pur, p, dE, gf2Basis);

    #  GUAVA's BCHCode generator derivation, at gf2's alpha.
    G := Indeterminate(F, 1) ^ 0;
    powerSet := [b0 .. b0 + delta - 2];
    while Length(powerSet) > 0 do
        test := powerSet[1] mod n;
        G := G * MinimalPolynomial(F, alpha ^ test, 1);
        t := (q * test) mod n;
        while t <> (test mod n) do
            RemoveSet(powerSet, t);
            t := (q * t) mod n;
        od;
        RemoveSet(powerSet, test);
    od;
    k := n - DegreeOfLaurentPolynomial(G);
    divides := IsZero((x ^ n - One(F)) mod G);

    #  The defining set, as the q-cyclotomic closure GUAVA takes.
    definingSet := [];
    for j in [b0 .. b0 + delta - 2] do
        t := j mod n;
        repeat
            AddSet(definingSet, t);
            t := (q * t) mod n;
        until t = (j mod n);
    od;
    rootsChecked := fail;
    if n <= NATIVE_CODE_LIMIT then
        rootsChecked := ForAll(definingSet, e -> IsZero(Value(G, alpha ^ e)))
            and Number([0 .. n - 1], e -> IsZero(Value(G, alpha ^ e)))
                = Length(definingSet);
    fi;

    #  GUAVA's own `BCHCode`, at GUAVA's own root, where the generator matrix
    #  fits. Its dimension and defining set are read off the object itself,
    #  the defining set as the exponents of GUAVA's root that its generator
    #  annihilates.
    if n <= NATIVE_CODE_LIMIT then
        code := BCHCode(n, b0, delta, F);
        bchPoly := GeneratorPol(code);
        bchGenerator := List(
            CoefficientsOfUnivariatePolynomial(bchPoly),
            c -> BaseIndexOfFFE(c, p, dB, baseBasis));
        bchMatches := bchPoly = G;
        bchK := Dimension(code);
        bchDefiningSet := Filtered([0 .. n - 1],
            e -> IsZero(Value(bchPoly, pur ^ e)));
        nativeCode := GeneratorPolCode(G, n, F);
        unavailable := fail;
        encoderNote := "GUAVA CodewordVector on GeneratorPolCode(G, n, F)";
    else
        code := fail;
        bchPoly := fail;
        bchGenerator := fail;
        bchMatches := fail;
        bchK := fail;
        bchDefiningSet := fail;
        nativeCode := fail;
        unavailable := Concatenation(
            "GUAVA GeneratorPolCode materializes a ", String(k), " x ",
            String(n), " generator matrix, which exhausts memory");
        encoderNote := "GUAVA cyclic-code encoding map c(x) = m(x) * G(x)";
    fi;

    native := [];
    systematic := [];
    bchNative := [];
    bchSystematic := [];
    if nativeCode = fail then crossChecked := fail; else crossChecked := true; fi;
    for messages in row.messages do
        symbols := DecodeSymbols(messages, row.k, q);
        messageFFE := List(symbols, c -> FFEFromIndex(c, p, dB, baseGen));
        message := UnivariatePolynomial(F, messageFFE, 1);
        product := message * G;
        coefficients := List(CoefficientsOfUnivariatePolynomial(product), c -> c);
        while Length(coefficients) < n do Add(coefficients, Zero(F)); od;
        Add(native, EncodeSymbols(
            List(coefficients, c -> BaseIndexOfFFE(c, p, dB, baseBasis)), q));
        if nativeCode <> fail then
            checkWord := VectorCodeword(messageFFE * nativeCode);
            crossChecked := crossChecked and
                List(checkWord, c -> BaseIndexOfFFE(c, p, dB, baseBasis))
                = List(coefficients, c -> BaseIndexOfFFE(c, p, dB, baseBasis));
        fi;
        shifted := x ^ (n - k) * message;
        remainder := shifted mod G;
        systematicPoly := shifted - remainder;
        coefficients := List(
            CoefficientsOfUnivariatePolynomial(systematicPoly), c -> c);
        while Length(coefficients) < n do Add(coefficients, Zero(F)); od;
        Add(systematic, EncodeSymbols(
            List(coefficients, c -> BaseIndexOfFFE(c, p, dB, baseBasis)), q));

        #  The same two encodings on GUAVA's own code object.
        if code <> fail then
            checkWord := VectorCodeword(CodewordVector(messageFFE, code));
            Add(bchNative, EncodeSymbols(
                List(checkWord, c -> BaseIndexOfFFE(c, p, dB, baseBasis)), q));
            shifted := x ^ (n - bchK) * message;
            remainder := shifted mod bchPoly;
            systematicPoly := shifted - remainder;
            coefficients := List(
                CoefficientsOfUnivariatePolynomial(systematicPoly), c -> c);
            while Length(coefficients) < n do Add(coefficients, Zero(F)); od;
            Add(bchSystematic, EncodeSymbols(
                List(coefficients, c -> BaseIndexOfFFE(c, p, dB, baseBasis)), q));
        fi;
    od;

    out := "";
    Append(out, Concatenation("    {\n      \"id\": ", JsonQuote(row.id), ",\n"));
    Append(out, Concatenation("      \"base_presentation\": ",
        JsonQuote(basePresentation), ",\n"));
    Append(out, Concatenation("      \"base_generator_image\": ",
        JsonMaybeInt(IndexOfFFE(baseGen, p, dB)), ",\n"));
    Append(out, Concatenation("      \"splitting_presentation\": ",
        JsonQuote(extPresentation), ",\n"));
    Append(out, Concatenation("      \"splitting_generator_image\": ",
        JsonMaybeInt(IndexOfFFE(extGen, p, dE)), ",\n"));
    Append(out, Concatenation("      \"alpha_index\": ",
        String(IndexOfFFE(alpha, p, dE)), ",\n"));
    Append(out, Concatenation("      \"alpha_gf2_index\": ",
        String(alphaGf2Index), ",\n"));
    Append(out, Concatenation("      \"guava_default_root_index\": ",
        String(IndexOfFFE(pur, p, dE)), ",\n"));
    Append(out, Concatenation("      \"guava_root_gf2_index\": ",
        String(guavaRootGf2Index), ",\n"));
    Append(out, Concatenation("      \"alpha_matches_guava_default\": ",
        JsonBool(alpha = pur), ",\n"));
    Append(out, Concatenation("      \"k\": ", String(k), ",\n"));
    Append(out, Concatenation("      \"defining_set\": ",
        JsonInts(definingSet), ",\n"));
    Append(out, Concatenation("      \"generator\": ", JsonInts(
        List(CoefficientsOfUnivariatePolynomial(G),
             c -> BaseIndexOfFFE(c, p, dB, baseBasis))), ",\n"));
    Append(out, Concatenation("      \"generator_divides_x_n_minus_one\": ",
        JsonBool(divides), ",\n"));
    Append(out, Concatenation("      \"defining_set_root_checked\": ",
        JsonMaybeBool(rootsChecked), ",\n"));
    if bchGenerator = fail then
        Append(out, "      \"bchcode_generator\": null,\n");
        Append(out, "      \"bchcode_generator_matches\": null,\n");
        Append(out, "      \"bchcode_k\": null,\n");
        Append(out, "      \"bchcode_defining_set\": null,\n");
        Append(out, "      \"bchcode_codewords_native\": null,\n");
        Append(out, "      \"bchcode_codewords_systematic\": null,\n");
        Append(out, Concatenation("      \"bchcode_unavailable\": ",
            JsonQuote(unavailable), ",\n"));
    else
        Append(out, Concatenation("      \"bchcode_generator\": ",
            JsonInts(bchGenerator), ",\n"));
        Append(out, Concatenation("      \"bchcode_generator_matches\": ",
            JsonBool(bchMatches), ",\n"));
        Append(out, Concatenation("      \"bchcode_k\": ",
            String(bchK), ",\n"));
        Append(out, Concatenation("      \"bchcode_defining_set\": ",
            JsonInts(bchDefiningSet), ",\n"));
        Append(out, Concatenation("      \"bchcode_codewords_native\": ",
            JsonStrings(bchNative), ",\n"));
        Append(out, Concatenation("      \"bchcode_codewords_systematic\": ",
            JsonStrings(bchSystematic), ",\n"));
        Append(out, "      \"bchcode_unavailable\": null,\n");
    fi;
    Append(out, Concatenation("      \"native_encoder\": ",
        JsonQuote(encoderNote), ",\n"));
    Append(out, Concatenation("      \"native_encoder_cross_checked\": ",
        JsonMaybeBool(crossChecked), ",\n"));
    Append(out, Concatenation("      \"systematic_rule\": ", JsonQuote(
        "x^r*m(x) - (x^r*m(x) mod G) over GUAVA's own generator"), ",\n"));
    Append(out, Concatenation("      \"codewords_native\": ",
        JsonStrings(native), ",\n"));
    Append(out, Concatenation("      \"codewords_systematic\": ",
        JsonStrings(systematic), "\n"));
    Append(out, "    }");
    return out;
end;;

#############################################################################
##  Entry point.

Main := function()
    local corpus, out, i, rows;
    corpus := JsonParse(StringFile(CORPUS));
    out := "{\n  \"oracle\": {\n";
    Append(out, Concatenation("    \"system\": ", JsonQuote("GAP with GUAVA"), ",\n"));
    Append(out, Concatenation("    \"gap_version\": ",
        JsonQuote(GAPInfo.Version), ",\n"));
    Append(out, Concatenation("    \"guava_version\": ",
        JsonQuote(InstalledPackageVersion("guava")), ",\n"));
    Append(out, Concatenation("    \"sonata_version\": ",
        JsonQuote(InstalledPackageVersion("sonata")), ",\n"));
    Append(out, Concatenation("    \"entry_point\": ",
        JsonQuote("GUAVA BCHCode(n, b, delta, F)"), "\n  },\n"));
    Append(out, Concatenation("  \"seed\": ", JsonQuote(corpus.seed), ",\n"));
    Append(out, "  \"rows\": [\n");
    rows := corpus.rows;
    for i in [1 .. Length(rows)] do
        Append(out, BuildRow(rows[i]));
        if i < Length(rows) then Append(out, ",\n"); else Append(out, "\n"); fi;
    od;
    Append(out, "  ]\n}\n");
    FileString(OUTPUT, out);
end;;

Main();
QUIT;
