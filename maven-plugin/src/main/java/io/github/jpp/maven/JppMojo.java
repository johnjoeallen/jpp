package io.github.jpp.maven;

import java.io.File;

import org.apache.maven.plugin.AbstractMojo;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugins.annotations.LifecyclePhase;
import org.apache.maven.plugins.annotations.Mojo;
import org.apache.maven.plugins.annotations.Parameter;
import org.apache.maven.project.MavenProject;

@Mojo(name = "generate", defaultPhase = LifecyclePhase.GENERATE_SOURCES, threadSafe = true)
public final class JppMojo extends AbstractMojo {
    @Parameter(defaultValue = "${project}", readonly = true, required = true)
    private MavenProject project;

    @Parameter(defaultValue = "${project.basedir}/src/main/jpp")
    private File inputDirectory;

    @Parameter(defaultValue = "${project.build.directory}/generated-sources/jpp")
    private File outputDirectory;

    @Parameter(property = "jpp.inPlace", defaultValue = "false")
    private boolean inPlace;

    @Parameter(property = "jpp.skip", defaultValue = "false")
    private boolean skip;

    @Override
    public void execute() throws MojoExecutionException {
        if (skip) {
            getLog().info("Skipping JPP generation.");
            return;
        }

        try {
            File sourceRoot = resolveSourceRoot();
            JppEngine.generate(inputDirectory.toPath(), sourceRoot.toPath());
            project.addCompileSourceRoot(sourceRoot.getAbsolutePath());
        } catch (JppException ex) {
            throw new MojoExecutionException(ex.getMessage(), ex);
        }
    }

    private File resolveSourceRoot() {
        if (!inPlace) {
            return outputDirectory;
        }

        if (inputDirectory.isFile()) {
            File parent = inputDirectory.getParentFile();
            return parent != null ? parent : inputDirectory;
        }

        return inputDirectory;
    }
}
