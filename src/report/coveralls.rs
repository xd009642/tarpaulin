use std::collections::HashMap;
use coveralls_api::*;
use tracer::TracerData;
use config::Config;

pub fn export(coverage_data: &[TracerData], config: &Config) {
    if let Some(ref key) = config.coveralls {
        let id = match config.ci_tool {
            Some(ref service) => Identity::ServiceToken(Service {
                service_name: service.clone(),
                service_job_id: key.clone()
            }),
            _ => Identity::RepoToken(key.clone()),
        };
        let mut report = CoverallsReport::new(id);
        let files = coverage_data.iter()
                                 .fold(vec![], |mut acc, x| {
                                     if !acc.contains(&x.path.as_path()) {
                                         acc.push(x.path.as_path());
                                     }
                                     acc    
                                 });
        
        for file in &files {
            let rel_path = config.strip_project_path(file);
            let mut lines: HashMap<usize, usize> = HashMap::new();
            let fcov = coverage_data.iter()
                                    .filter(|x| x.path == *file)
                                    .collect::<Vec<&TracerData>>();

            for c in &fcov {
                lines.insert(c.line as usize, c.hits as usize);
            }
            if let Ok(source) = Source::new(&rel_path, file, &lines, &None, false) {
                report.add_source(source);
            }
        }
        let res = report.send_to_coveralls();
        if config.verbose {
            match res {
                Ok(_) => {},
                Err(e) => println!("Coveralls send failed. {}", e),
            }
        }
    } else {
        panic!("No coveralls key specified.");
    }
}
