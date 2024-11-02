use anyhow::{Context, Result};
use bollard::container::RemoveContainerOptions;
use bollard::{
    container::StopContainerOptions, image::CreateImageOptions, Docker, API_DEFAULT_VERSION,
};
use futures::TryStreamExt;
use tracing::{debug, info, warn};
use version_compare::Version;

const ERROR_MESSAGE: &str = "Docker is not available, confirm it is installed and running";

/// This function returns a Docker client. Before returning, it confirms that it can
/// actually query the API and checks that the API version is sufficient. It first
/// tries to connect at the default socket location and if that fails, it tries to find
/// a socket in the user's home directory. On Windows NT it doesn't try that since
/// there no second location, there is just the one named pipe.
pub async fn get_docker() -> Result<Docker> {
    let docker = Docker::connect_with_local_defaults()
        .context(format!("{} (init_default)", ERROR_MESSAGE))?;

    // We have to specify the type because the compiler can't figure out the error
    // in the case where the system is Unix.
    let out: Result<(Docker, bollard::system::Version), bollard::errors::Error> =
        match docker.version().await {
            Ok(version) => Ok((docker, version)),
            Err(err) => {
                debug!(
                    "Received this error trying to use default Docker socket location: {:#}",
                    err
                );
                // Look for the socket in ~/.docker/run
                // We don't have to do this if this issue gets addressed:
                // https://github.com/fussybeaver/bollard/issues/345
                #[cfg(unix)]
                {
                    let path = dirs::home_dir()
                        .context(format!("{} (home_dir)", ERROR_MESSAGE))?
                        .join(".docker")
                        .join("run")
                        .join("docker.sock");
                    debug!("Looking for Docker socket at {}", path.display());
                    let path = path.to_str().context(format!("{} (path)", ERROR_MESSAGE))?;
                    let docker = Docker::connect_with_socket(path, 120, API_DEFAULT_VERSION)
                        .context(format!("{} (init_home)", ERROR_MESSAGE))?;
                    let version = docker
                        .version()
                        .await
                        .context(format!("{} (version_home)", ERROR_MESSAGE))?;
                    Ok((docker, version))
                }
                // Just return the original error.
                #[cfg(not(unix))]
                Err(err)
            }
        };
    let (docker, version) = out?;

    // Try to warn the user about their Docker version being too old. We don't error
    // out if the version is too old in case we're wrong about the minimum version
    // for their particular system. We just print a warning.
    match version.api_version {
        Some(current_api_version) => match Version::from(&current_api_version) {
            Some(current_api_version) => {
                let minimum_api_version = Version::from("1.42").unwrap();
                if current_api_version < minimum_api_version {
                    eprintln!(
                            "WARNING: Docker API version {} is too old, minimum required version is {}. Please update Docker!",
                            current_api_version,
                            minimum_api_version,
                        );
                } else {
                    debug!("Docker version is sufficient: {}", current_api_version);
                }
            }
            None => {
                eprintln!(
                    "WARNING: Failed to parse Docker API version: {}",
                    current_api_version
                );
            }
        },
        None => {
            eprintln!(
                "WARNING: Failed to determine Docker version, confirm your Docker is up to date!"
            );
        }
    }

    Ok(docker)
}

/// Delete a container. If the container doesn't exist, that's fine, just move on.
pub async fn delete_container(docker: &Docker, container_name: &str) -> Result<()> {
    info!(
        "Removing container with name {} (if it exists)",
        container_name
    );

    let options = Some(RemoveContainerOptions {
        force: true,
        ..Default::default()
    });

    // Ignore any error, it'll be because the container doesn't exist.
    let result = docker.remove_container(container_name, options).await;

    match result {
        Ok(_) => info!("Succesfully removed container {}", container_name),
        Err(err) => warn!(
            "Failed to remove container {}: {:#} (it probably didn't exist)",
            container_name, err
        ),
    }

    Ok(())
}

/// Stop a container. If the container doesn't exist, that's fine, just move on.
pub async fn stop_container(docker: &Docker, container_name: &str) -> Result<()> {
    info!(
        "Stopping container with name {} (if it exists)",
        container_name
    );

    let options = Some(StopContainerOptions {
        // Timeout in seconds before we kill the container.
        t: 1,
    });

    // Ignore any error, it'll be because the container doesn't exist.
    let result = docker.stop_container(container_name, options).await;

    match result {
        Ok(_) => info!("Succesfully stopped container {}", container_name),
        Err(err) => warn!(
            "Failed to stop container {}: {:#} (it probably didn't exist)",
            container_name, err
        ),
    }

    Ok(())
}

pub async fn pull_docker_image(docker: &Docker, image_name: &str) -> Result<()> {
    debug!("Checking if we have to pull docker image {}", image_name);

    let options = Some(CreateImageOptions {
        from_image: image_name,
        ..Default::default()
    });

    // Check if the image is there. If it is, exit early, the user can update any
    // images we've already pulled manually if they want.
    if docker.inspect_image(image_name).await.is_ok() {
        debug!(
            "Image {} found locally, not attempting to pull it",
            image_name
        );
        return Ok(());
    }

    // The image is not present, let the user know we'll pull it.
    info!("Image {} not found, pulling it now...", image_name);

    // The docker pull CLI command is just sugar around this API.
    docker
        .create_image(options, None, None)
        // Just wait for the whole stream, we don't need to do other things in parallel.
        .try_collect::<Vec<_>>()
        .await
        .with_context(|| format!("Failed to pull image {}", image_name))?;

    info!("Pulled docker image {}", image_name);

    Ok(())
}
