package demo;

import java.time.Instant;
import java.util.UUID;

public class Customer {

    private UUID id;

    public UUID id() {
        return this.id;
    }

    public synchronized Customer id(UUID value) {
        if (value == null) {
            throw new NullPointerException(
                "Property 'id' cannot be null"
            );
        }

        if (this.id != null) {
            throw new IllegalStateException(
                "Property 'id' is once-only"
            );
        }

        this.id = value;

        return this;
    }


    private final Instant created;

    public Customer(
        Instant created
    ) {
        this.created = created;
    }

    public Instant created() {
        return this.created;
    }


    private String firstName;

    public String firstName() {
        return this.firstName;
    }

    public Customer firstName(String value) {
        value = value.trim();

        this.firstName = value;

        return this;
    }


    private String lastName;

    public String lastName() {
        return this.lastName;
    }

    public Customer lastName(String value) {
        value = value.trim();

        this.lastName = value;

        return this;
    }


    private String email;

    public String email() {
        return this.email;
    }

    public Customer email(String value) {
        value = value.trim().toLowerCase();

        this.email = value;

        return this;
    }


    private Customer referrer;

    public Customer referrer() {
        return this.referrer;
    }

    public Customer referrer(Customer value) {
        this.referrer = value;

        return this;
    }



    public String referrerName() {
        return java.util.Optional.ofNullable(referrer).map(__jpp_value -> __jpp_value.fullName()).orElse("");
    }


    public CustomerSummary summary() {
        CustomerSummary target = new CustomerSummary();

        target.displayName(fullName());
        target.referrerName(java.util.Optional.ofNullable(referrer).map(__jpp_value -> __jpp_value.fullName()).orElse(""));
        target.email(email());

        return target;
    }



    public String fullName() {
        return firstName()
            + " "
            + lastName();
    }


    public static class CustomerSummary {

        private String displayName;
        private String referrerName;
        private String email;

        public String displayName() {
            return this.displayName;
        }

        public CustomerSummary displayName(String value) {
            this.displayName = value;
            return this;
        }

        public String referrerName() {
            return this.referrerName;
        }

        public CustomerSummary referrerName(String value) {
            this.referrerName = value;
            return this;
        }

        public String email() {
            return this.email;
        }

        public CustomerSummary email(String value) {
            this.email = value;
            return this;
        }
    }
}
