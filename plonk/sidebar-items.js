initSidebarItems({"mod":[["commitment_scheme","Ideally we should cleanly abstract away the polynomial commitment scheme We note that PLONK makes use of the linearisation technique   conceived in SONIC [Mary Maller]. This technique implicitly requires the commitment scheme to be homomorphic. `Merkle Tree like` techniques such as FRI are not homomorphic and therefore for PLONK to be usable with all commitment schemes without modification, one would need to remove the lineariser"],["constraint_system","The constraint System module stores the implementation of the PLONK Standard Composer, as well as the circuit tools and abstractions, used by the Composer to generate, build, preprocess & prove constructed circuits."],["fft","FFT module contains the tools needed by the Composer backend to know and use the logic behind Polynomials. As well as the operations that the `Composer` needs to peform with them."],["notes","This module is a self contained file which explains how PLONK and its protocol components work in our library."],["proof_system","proving system"],["transcript","This is an extension over the Merlin Transcript which adds a few extra functionalities."]]});