use crate::estimates::codec::{
    Decoder, Encoder, invalid, put_appetite, put_ratio, take_appetite, take_ratio,
};
use crate::signals::context_codec::{
    put_hours_permission, put_investor_profile, take_hours_permission, take_investor_profile,
};
use crate::{Database, sql::settings as sql};
use application::{SettingsRepository, SystemPortError, WriteCompleted};
use domain::{
    InvestorProfile, OrderPermission, ProfileWeight, RiskSettings, SavedMarketOrderSetting,
    WeightCaps,
};
use rusqlite::{Connection, OptionalExtension, Transaction};

const PROFILE: &str = "investor_profile";
const RISK: &str = "risk_settings";
const TOSS_SPEC: &str = "toss_spec_hash";
const CURRENT_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossSpecHash(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossSpecHashError {
    InvalidFormat,
}

impl TossSpecHash {
    pub fn try_new(value: &str) -> Result<Self, TossSpecHashError> {
        if value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(TossSpecHashError::InvalidFormat)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct SettingsStore<'database> {
    database: &'database Database,
}

impl<'database> SettingsStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }

    pub async fn store_toss_spec_hash(
        &self,
        hash: &TossSpecHash,
    ) -> Result<WriteCompleted, SystemPortError> {
        let payload = hash.as_str().to_owned();
        self.database
            .write(move |transaction| put(transaction, TOSS_SPEC, &payload))
            .await
    }

    pub async fn toss_spec_hash(&self) -> Result<Option<TossSpecHash>, SystemPortError> {
        self.database
            .read(|connection| {
                read_current(connection, TOSS_SPEC)?
                    .map(|payload| TossSpecHash::try_new(&payload).map_err(invalid))
                    .transpose()
            })
            .await
    }
}

fn put(
    transaction: &Transaction<'_>,
    name: &str,
    payload: &str,
) -> rusqlite::Result<WriteCompleted> {
    match transaction.execute(
        sql::UPSERT,
        rusqlite::params![name, CURRENT_VERSION, payload],
    )? {
        1 => Ok(WriteCompleted),
        _ => Err(invalid(())),
    }
}

fn upgrade(version: i64, payload: String) -> rusqlite::Result<String> {
    match version {
        CURRENT_VERSION => Ok(payload),
        _ => Err(invalid(())),
    }
}

fn read_current(connection: &Connection, name: &str) -> rusqlite::Result<Option<String>> {
    let stored: Option<(i64, String)> = connection
        .query_row(sql::FIND, [name], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    stored
        .map(|(version, payload)| upgrade(version, payload))
        .transpose()
}

fn encode_profile(profile: &InvestorProfile) -> String {
    let mut out = Encoder::new();
    put_investor_profile(&mut out, profile);
    out.finish()
}

fn decode_profile(payload: &str) -> rusqlite::Result<InvestorProfile> {
    let mut input = Decoder::new(payload);
    let profile = take_investor_profile(&mut input)?;
    input.finish()?;
    Ok(profile)
}

const fn order_tag(permission: OrderPermission) -> &'static str {
    match permission {
        OrderPermission::Disabled => "disabled",
        OrderPermission::Enabled => "enabled",
    }
}

const fn market_tag(setting: SavedMarketOrderSetting) -> &'static str {
    match setting {
        SavedMarketOrderSetting::Disabled => "disabled",
        SavedMarketOrderSetting::Enabled => "enabled",
    }
}

fn encode_risk(settings: RiskSettings) -> String {
    let mut out = Encoder::new();
    out.push(order_tag(settings.orders()));
    out.push(market_tag(settings.market_orders()));
    put_hours_permission(&mut out, settings.hours());
    put_ratio(&mut out, settings.weight_caps().instrument_cap().as_ratio());
    put_ratio(
        &mut out,
        settings.weight_caps().instrument_warning().as_ratio(),
    );
    put_ratio(&mut out, settings.weight_caps().sector_cap().as_ratio());
    put_appetite(&mut out, settings.risk_appetite());
    out.finish()
}

fn take_weight(input: &mut Decoder<'_>) -> rusqlite::Result<ProfileWeight> {
    ProfileWeight::try_new(take_ratio(input)?).map_err(invalid)
}

fn decode_risk(payload: &str) -> rusqlite::Result<RiskSettings> {
    let mut input = Decoder::new(payload);
    let orders = match input.next()? {
        "disabled" => OrderPermission::Disabled,
        "enabled" => OrderPermission::Enabled,
        _ => return Err(invalid(())),
    };
    let market = match input.next()? {
        "disabled" => SavedMarketOrderSetting::Disabled,
        "enabled" => SavedMarketOrderSetting::Enabled,
        _ => return Err(invalid(())),
    };
    let hours = take_hours_permission(&mut input)?;
    let caps = WeightCaps::try_new(
        take_weight(&mut input)?,
        take_weight(&mut input)?,
        take_weight(&mut input)?,
    )
    .map_err(invalid)?;
    let appetite = take_appetite(&mut input)?;
    input.finish()?;
    Ok(RiskSettings::new(orders, market, hours, caps, appetite))
}

fn stored_profile(connection: &Connection) -> rusqlite::Result<Option<InvestorProfile>> {
    read_current(connection, PROFILE)?
        .map(|payload| decode_profile(&payload))
        .transpose()
}

fn stored_risk(connection: &Connection) -> rusqlite::Result<Option<RiskSettings>> {
    read_current(connection, RISK)?
        .map(|payload| decode_risk(&payload))
        .transpose()
}

fn risk_with_profile(previous: Option<RiskSettings>, profile: &InvestorProfile) -> RiskSettings {
    RiskSettings::new(
        previous.map_or(OrderPermission::Disabled, RiskSettings::orders),
        previous.map_or(
            SavedMarketOrderSetting::Disabled,
            RiskSettings::market_orders,
        ),
        profile.hours_permission().state(),
        profile.weight_caps(),
        profile.risk_appetite(),
    )
}

fn profile_with_risk(
    previous: Option<&InvestorProfile>,
    settings: RiskSettings,
) -> rusqlite::Result<InvestorProfile> {
    let fallback = InvestorProfile::default();
    let mut saved = previous.unwrap_or(&fallback).saved_state();
    saved.hours_permission = settings.hours();
    saved.risk_appetite = settings.risk_appetite();
    saved.instrument_cap = settings.weight_caps().instrument_cap().as_ratio();
    saved.instrument_warning = settings.weight_caps().instrument_warning().as_ratio();
    saved.sector_cap = settings.weight_caps().sector_cap().as_ratio();
    InvestorProfile::restore_from_saved_state(saved).map_err(invalid)
}

fn write_profile(
    transaction: &Transaction<'_>,
    profile: &InvestorProfile,
) -> rusqlite::Result<WriteCompleted> {
    let risk = risk_with_profile(stored_risk(transaction)?, profile);
    put(transaction, RISK, &encode_risk(risk))?;
    put(transaction, PROFILE, &encode_profile(profile))
}

fn write_risk(
    transaction: &Transaction<'_>,
    settings: RiskSettings,
) -> rusqlite::Result<WriteCompleted> {
    let profile = profile_with_risk(stored_profile(transaction)?.as_ref(), settings)?;
    put(transaction, PROFILE, &encode_profile(&profile))?;
    put(transaction, RISK, &encode_risk(settings))
}

impl SettingsRepository for SettingsStore<'_> {
    async fn store_profile(
        &self,
        profile: &InvestorProfile,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned = profile.clone();
        self.database
            .write(move |transaction| write_profile(transaction, &owned))
            .await
    }

    async fn profile(&self) -> Result<Option<InvestorProfile>, SystemPortError> {
        self.database.read(stored_profile).await
    }

    async fn store_risk_settings(
        &self,
        settings: &RiskSettings,
    ) -> Result<WriteCompleted, SystemPortError> {
        let owned = *settings;
        self.database
            .write(move |transaction| write_risk(transaction, owned))
            .await
    }

    async fn risk_settings(&self) -> Result<Option<RiskSettings>, SystemPortError> {
        self.database.read(stored_risk).await
    }
}
