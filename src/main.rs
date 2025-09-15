use clap::Parser;
use db_test::{
    benchmark::BenchmarkRunner,
    config::BenchConfig,
    error::Result,
};

fn main() -> Result<()> {
    // Parse command line arguments
    let config = BenchConfig::parse();
    
    // Print configuration summary
    config.print_summary();
    
    // Create and run benchmark
    let mut runner = BenchmarkRunner::new(config);
    let results = runner.run()?;
    
    // Print results
    for result in &results {
        result.print_summary();
    }
    
    // Optionally save results to JSON file
    if results.len() == 1 {
        let json_output = results[0].to_json()?;
        let filename = format!(
            "benchmark_results_{}_{}.json",
            results[0].backend.to_lowercase(),
            results[0].test_mode.to_lowercase()
        );
        std::fs::write(&filename, json_output)?;
        println!("Results saved to: {}", filename);
    } else {
        // Save multiple results
        for (i, result) in results.iter().enumerate() {
            let json_output = result.to_json()?;
            let filename = format!(
                "benchmark_results_{}_{}_{}.json",
                result.backend.to_lowercase(),
                result.test_mode.to_lowercase(),
                i
            );
            std::fs::write(&filename, json_output)?;
            println!("Results saved to: {}", filename);
        }
    }
    
    Ok(())
}
