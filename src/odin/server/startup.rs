use std::process::exit;
use std::{io, process::Child};

use daemonize::{Daemonize, Error};
use log::{debug, error, info};

use crate::mods::bepinex::BepInExEnvironment;
use crate::notifications::enums::event_status::EventStatus;
use crate::notifications::enums::notification_event::NotificationEvent;
use crate::utils::common_paths::{game_directory, saves_directory};
use crate::utils::environment::fetch_var;
use crate::{
  constants,
  executable::create_execution,
  files::{config::ValheimArguments, create_file},
  messages,
  utils::environment,
};

type CommandResult = io::Result<Child>;

pub fn start_daemonized(config: ValheimArguments) -> Result<CommandResult, Error> {
  debug!("Starting server daemonized...");
  let stdout = create_file(format!("{}/logs/valheim_server.log", game_directory()).as_str());
  let stderr = create_file(format!("{}/logs/valheim_server.err", game_directory()).as_str());
  let command = start(config);
  Daemonize::new()
    .working_directory(game_directory())
    .user("steam")
    .group("steam")
    .stdout(stdout)
    .stderr(stderr)
    .privileged_action(|| {
      let bepinex_env = BepInExEnvironment::new();
      if bepinex_env.is_installed() {
        info!("Server has been started with BepInEx! Keep in mind this may cause errors!!");
        messages::modding_disclaimer();
        debug!("{:#?}", bepinex_env);
      }
      info!("Server has been started and Daemonize. It should be online shortly!");
      info!("Keep an eye out for 'Game server connected' in the log!");
      NotificationEvent::Start(EventStatus::Successful).send_notification();
      info!("(this indicates its online without any errors.)")
    })
    .privileged_action(move || command)
    .start()
}

pub fn start(config: ValheimArguments) -> CommandResult {
  let mut command = create_execution(&config.command);

  debug!("--------------------------------------------------------------------------------------------------------------");

  let ld_library_path_value = environment::fetch_multiple_var(
    constants::LD_LIBRARY_PATH_VAR,
    format!("{}/linux64", game_directory()).as_str(),
  );
  debug!("Setting up base command");
  debug!("Launching With Args: \n{:#?}", &config);
  // Sets the base command for the server
  let base_command = command
    .env("SteamAppId", fetch_var("APPID", "892970"))
    .current_dir(game_directory());

  // Sets the name of the server, (Can be set with ENV variable NAME)
  let name = fetch_var("NAME", &config.name);
  debug!("Setting name to: {}", &name);
  base_command.arg("-name");
  base_command.arg(&name);

  // Sets the port of the server, (Can be set with ENV variable PORT)
  let port = fetch_var("PORT", &config.port);
  debug!("Setting port to: {}", &port);
  base_command.args(["-port", &port]);

  // Sets the world of the server, (Can be set with ENV variable WORLD)
  let world = fetch_var("WORLD", &config.world);
  debug!("Setting world to: {}", &fetch_var("WORLD", &world));
  base_command.arg("-world");
  base_command.arg(&world);

  // Determines if the server is public or not
  let public = fetch_var("PUBLIC", config.public.as_str());
  debug!("Setting public to: {}", &public);
  base_command.args(["-public", &public]);

  // Sets the save interval in seconds
  if let Some(save_interval) = &config.save_interval {
    let interval = save_interval.to_string();
    debug!("Setting save interval to: {}", &interval);
    base_command.args(["-saveinterval", &interval]);
  };

  // Add set_key to the command
  if let Some(set_key) = &config.set_key {
    debug!("Setting set_key to: {}", &set_key);
    base_command.args(["-setkey", &set_key]);
  };

  // Add preset to the command
  if let Some(preset) = &config.preset {
    debug!("Setting preset to: {}", &preset);
    base_command.args(["-preset", &preset]);
  };

  // Add modifiers to the command
  if let Some(modifiers) = &config.modifiers {
    modifiers.iter().for_each(|modifier| {
      debug!(
        "Setting modifier to: {} {}",
        &modifier.name, &modifier.value
      );
      base_command.args(["-modifier", &modifier.name, &modifier.value]);
    });
  };

  // Extra args for the server
  base_command.args({
    format!(
      "-nographics -batchmode {}",
      fetch_var("SERVER_EXTRA_LAUNCH_ARGS", "")
    )
    .trim()
    .to_string()
    .split(' ')
    .collect::<Vec<&str>>()
  });

  let is_public = config.public.eq("1");
  let is_vanilla = fetch_var("TYPE", "vanilla").eq_ignore_ascii_case("vanilla");
  let no_password = config.password.is_empty();

  // If no password env variable
  if !is_public && !is_vanilla && no_password {
    info!("No password found, skipping password flag.")
  } else if no_password && (is_public || is_vanilla) {
    error!("Cannot run you server with no password! PUBLIC must be 0 and cannot be a Vanilla type server.");
    exit(1)
  } else {
    info!("Password found, adding password flag.");
    base_command.arg("-password");
    base_command.arg(&config.password);
  }

  if fetch_var("ENABLE_CROSSPLAY", "0").eq("1") {
    info!("Launching with Crossplay! <3");
    base_command.arg("-crossplay");
  } else {
    info!("No Crossplay Enabled!")
  }

  // Tack on save dir at the end.
  base_command.args(["-savedir", &saves_directory()]);

  debug!("Base Command: {:#?}", base_command);

  debug!("Executable: {}", &config.command);
  info!("Launching Command...");
  let bepinex_env = BepInExEnvironment::new();
  if bepinex_env.is_installed() {
    info!("BepInEx detected! Switching to run with BepInEx...");
    info!("BepInEx Environment: \n{:#?}", bepinex_env);
    bepinex_env.launch(base_command)
  } else {
    info!("Everything looks good! Running normally!");
    base_command
      .env(constants::LD_LIBRARY_PATH_VAR, ld_library_path_value)
      .spawn()
  }
}
