# wedding-rs-vp

## For Blake & Mellie's Wedding

https://www.blakeandmellie.wedding/

This repository holds the source code to the RSVP portal on our wedding website. It is the first project I've done in the Rust programming language, so it might be a little rough in places.

Things to know about certain pieces:

* The main website runs on [the Ghost blogging platform](https://ghost.org/), so some of the static assets are based on their [Casper theme](https://github.com/TryGhost/Casper).
* [Rocket](https://rocket.rs/) was chosen as the web framework as it's one of the more popular and better documented options.
* [Diesel](https://diesel.rs/) was used as an ORM, so this can work with most databases.
* The two Python scripts have to do with a spreadsheet we made for populating the database initially.
* The admin portal doesn't have built in authentication, but basic auth can be provided by the reverse proxy. We don't expose this endpoint to the internet at all.
* [Redis](https://redis.io/) is required for the bruteforce protections. This includes guests with a valid invite code trying to be silly and mess up your inbox by changing their dietary restriction a thousand times.
* I was hesitant to publish this at all because I invited too many hackers to my wedding.

I may or may not get around to making this an application suited for general deployment by the public. As is, it is very clearly just our site.