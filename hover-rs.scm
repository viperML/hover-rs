(define-module (hover-rs)
  #:use-module (guix build-system cargo)
  #:use-module (guix gexp)
  #:use-module ((guix licenses) #:prefix license:)
  #:use-module (guix packages)
  #:use-module (guix git-download)
  #:use-module (guix utils)
  #:use-module (gnu packages crates-io))

(define vcs-file?
  (or (git-predicate (current-source-directory))
      (const #t)))

(define local-source
  (local-file "." "source"
              #:recursive? #t
              #:select? (git-predicate (current-source-directory))))

(define-public hover-rs
  (package
    (name "hover-rs")
    (version "0.1.0")
    (source local-source)
    (build-system cargo-build-system)
    (arguments
      `(#:cargo-inputs
        (("rust-nix" ,rust-nix-0.27)
         ("rust-caps" ,rust-caps-0.5)
         ("rust-clap" ,rust-clap-4)
         ("rust-color-eyre" ,rust-color-eyre-0.6)
         ("rust-eyre" ,rust-eyre-0.6)
         ("rust-libc" ,rust-libc-0.2)
         ("rust-owo-colors" ,rust-owo-colors-3)
         ("rust-rand" ,rust-rand-0.8)
         ("rust-time" ,rust-time-0.3)
         ("rust-tracing" ,rust-tracing-0.1)
         ("rust-tracing-subscriber" ,rust-tracing-subscriber-0.3))))
    (home-page "")
    (synopsis "")
    (description "")
    (license license:expat)))
