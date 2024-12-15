Midnight Commander clone (Rust + Ratatui)
-----------------------------------------
Tento projekt funguje jako prohlížeč souborů na vzdáleném stroji.
Je zde obsažena základní funkcionalita inspirovaná právě MC, jako například pohyb mezi složkami, vytvoření nové složky,
nebo například úprava souboru.
Architektura je rozdělena na Client-Server, přičemž je server umístěn na onom vzdáleném stroji, klient je umístěn lokálně.
Klient posílá na server požadavky, server tyto požadavky zpracovává a odpovídá klientovi. Klient je zodpovědný za handlování uživatelských
inputů, za vykreslování UI a za celkovou logiku procházení UI.
Po připojení klienta na server se objeví dvě okna vedle sebe, tato aplikace nepodporuje myš, takže pohyb je umožněn pouze pomocí klávesnice
- jednotlivé commandy budou popsány níže.
Server pro každého klienta vytváří nový task (pomocí tokio crate), tím se sníží overhead vytváření nového vlákna pro každého nového klienta.
Jelikož chceme zachovat firewall u obou strojů jak lokálního tak vzdáleného ideálně beze změny, rozhodl jsem se použít OpenSSH server na straně
serveru (strana vzdáleného stroje). Tento SSH server nám umožnil to, že nemusíme otevírat firewall pro určité porty a zároveň můžeme komunikaci
ze strany klienta přímo "tunnelovat" přes SSH (pomocí ssh2 crate). 

COMMANDY
- Procházení souborů
	'f' -> změna z menu do procházení složky a naopak
	'x' -> změna oken (pravé za levé a naopak)
	'šipka vpravo' -> přechod do vnořené složky
	'šipka vlevo' -> přechod do předchozí složky
	'šipka nahoru' -> soubor/složka nad aktuální
	'šipka dolu' -> soubor/složka pod aktuální
	'escape' -> ukončení klienta
- Procházení menu
	'šipky nahoru, dolu' -> procházení menu
	'f' -> změna z menu do procházení složky a naopak
	'x' -> změna oken (pravé za levé a naopak)
	'enter' -> vybrání nějaké funkcionality z menu
	'escape' -> ukončení klienta
- Po vybrání nějakého menu
	'a-z, A-Z, ...' -> psaní do input_fieldu (pokud jde o nějaké menu vyžadující vstup)
	'backspace' -> smazání posledního znaku
	'enter' -> potvrzení input_fieldu -> zčervená pokud je input_field nesprávný
	'tab' -> změna input_fieldu pokud dané menu obsahuje více než jeden
	'escape'-> smazání input_fieldu (input_field zmizí, jakobychom ho nikdy nevybrali)
- Prohlížení souboru (menu View file)
	'šipky nahoru, dolu, vlevo, pravo' -> procházení obsahu souboru
	'esc' -> zrušení procházení, návrat do "Procházení souborů"
	'x' -> změna oken (pravé za levé a naopak)
	'i' -> editovací mód
- Editovací mód
	'a-z, A-Z, ...' -> psaní do souboru
	'backspace' -> smazání posledního znaku
	'enter' -> newline
	'šipky nahoru, dolu, vlevo, pravo' -> procházení obsahu souboru
	'esc' -> zrušení procházení, návrat do "Procházení souborů"
	'CTRL+S' -> uložení souboru a návrat do "Procházení souborů"

MÉ NASTAVENÍ
Server - windows 10 + wsl 1 (notebook 2)
       - WSL 1 je důležité, jestlikož WSL 2 má virtuální IP adresu na kterou se horko težko připojuje
       - OpenSSL instalované pomocí "sudo apt install openssh-server"
       - Důležitá nastavení v OpenSSL
		- AllowTCPForwarding yes
		- PermitTunnel yes
		- Ostatní nastavení by mělo být totožné s defaultním
	- Pokud by firewall na straně serveru nechtěl přijmout spojení:
		- ve Windows firewall zablokovat blokování příchozích TCP pro "sshd" - měl jsem ho defaulně jako "allow"
	- pak stačí server nastartovat klasicky přes "cargo run --bin server" popřípadě pokud máme pouze spustitelný soubor tak "./server"
Klient (notebook 1)
	- Uvnitř klientského kódu je přihl. jméno a heslo na SSH server - popřípadě nutno dát své
	- Klient se spouští příkazem "cargo run --bin client REMOTE 192.168.100.111 /mnt/c/Users/sisin"
	- Parametr REMOTE říká, že se připojujeme na vzdálený stroj - lokální funkcionalita nefunguje, byla odebrána z důvodu vzdálené funkcionality
	- Další parametr je IP adresa daného serveru, na který se připojujeme
	- Poslední parametr je defaultní directory, které se nám zobrazí

github: https://github.com/Bijecek/PVR_projekt
