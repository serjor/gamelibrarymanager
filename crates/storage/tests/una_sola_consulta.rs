//! La biblioteca entera sale en **una** consulta, con mil juegos o con uno.
//!
//! Antes esto se comprobaba cronometrando: «mil juegos en menos de 500 ms». No
//! servía por los dos lados. Fallaba una de cada seis veces con la máquina
//! ocupada compilando, porque el tiempo de reloj mide la carga del ordenador
//! tanto como el código; y aun así no habría cazado lo que decía vigilar,
//! porque mil consultas a un SQLite local caben de sobra en medio segundo.
//!
//! Ahora se cuenta lo que de verdad importa. sqlx registra cada sentencia que
//! ejecuta en el objetivo `sqlx::query`, así que basta con instalar un logger
//! que las cuente: una consulta por juego se ve como 1000 y no como «va lento
//! hoy».
//!
//! Este test vive solo en su fichero a propósito, y tiene que seguir solo. El
//! driver de SQLite ejecuta las sentencias en un hilo suyo, así que el contador
//! tiene que ser global; y un contador global solo es fiable si no hay otro test
//! lanzando consultas a la vez. Cada fichero de tests es un binario aparte, pero
//! dentro de uno los tests corren en paralelo: añadir aquí un segundo test que
//! toque la base haría que este empezara a fallar a ratos, que es justo de lo
//! que se venía huyendo.

use std::sync::atomic::{AtomicUsize, Ordering};

use domain::{
    EntryKind, Game, GameId, GameLink, LinkMethod, StoreAccount, StoreAccountId, StoreEntry,
    StoreEntryId, StoreId,
};
use storage::Database;
use storage::repositories::{
    GameLinkRepository, GameRepository, LibraryRepository, StoreAccountRepository,
    StoreEntryRepository,
};
use time::OffsetDateTime;

static CONSULTAS: AtomicUsize = AtomicUsize::new(0);

struct ContadorDeConsultas;

impl log::Log for ContadorDeConsultas {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target() == "sqlx::query"
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            CONSULTAS.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {}
}

static CONTADOR: ContadorDeConsultas = ContadorDeConsultas;

/// Deja el contador a cero y empieza a contar desde aquí.
fn empezar_a_contar() {
    // Si ya estaba instalado da igual: lo que importa es el cero de después.
    let _ = log::set_logger(&CONTADOR);
    // sqlx no registra nada si el nivel no está habilitado.
    log::set_max_level(log::LevelFilter::Debug);
    CONSULTAS.store(0, Ordering::Relaxed);
}

fn consultas_hechas() -> usize {
    CONSULTAS.load(Ordering::Relaxed)
}

fn juego(title: &str) -> Game {
    Game {
        id: GameId::new(),
        canonical_title: title.to_owned(),
        sort_title: title.to_lowercase(),
        igdb_id: None,
        cover_url: None,
        summary: None,
        released_at: None,
        genres: vec!["Shooter".to_owned()],
    }
}

#[tokio::test]
async fn mil_juegos_salen_en_una_sola_consulta() {
    let db = Database::in_memory().await.expect("base");
    let cuenta = StoreAccountRepository(&db)
        .upsert(&StoreAccount {
            id: StoreAccountId::new(),
            store: StoreId::Steam,
            account_ref: "cuenta".to_owned(),
            display_name: None,
            connected_at: OffsetDateTime::now_utc(),
            last_sync_at: None,
        })
        .await
        .expect("cuenta");

    let mut entradas = Vec::with_capacity(1000);
    let mut enlaces = Vec::with_capacity(1000);
    for i in 0..1000 {
        let ficha = juego(&format!("Juego {i:04}"));
        GameRepository(&db).upsert(&ficha).await.expect("ficha");
        let entrada = StoreEntry {
            id: StoreEntryId::new(),
            account_id: cuenta,
            store: StoreId::Steam,
            store_app_id: i.to_string(),
            kind: EntryKind::Owned,
            title: format!("Juego {i:04}"),
            playtime_minutes: Some(i),
            acquired_at: None,
            cover_url: None,
            store_url: None,
            raw: serde_json::json!({}),
        };
        enlaces.push(GameLink {
            game_id: ficha.id,
            store_entry_id: entrada.id,
            confidence: 1.0,
            method: LinkMethod::Auto,
        });
        entradas.push(entrada);
    }
    StoreEntryRepository(&db)
        .upsert_many(&entradas)
        .await
        .expect("entradas");
    GameLinkRepository(&db)
        .rebuild_auto(&enlaces)
        .await
        .expect("enlaces");

    // A partir de aquí es cuando cuenta.
    empezar_a_contar();
    let rows = LibraryRepository(&db).all().await.expect("biblioteca");
    let hechas = consultas_hechas();

    assert_eq!(rows.len(), 1000);
    assert!(
        rows[0].sort_title < rows[999].sort_title,
        "vienen ordenados"
    );
    // Un `1` exacto vale además como comprobación del propio contador: si el
    // logger no llegara a instalarse, esto saldría 0 y el test caería igual.
    assert_eq!(
        hechas, 1,
        "la biblioteca entera tiene que salir en una consulta; {hechas} sentencias \
         para mil juegos significa que alguien ha metido una consulta por juego"
    );
}
