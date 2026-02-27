<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="ids" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="formation|dispatch|application">
    <xsl:variable name="position" select="count(preceding::*[name()!='void' and name()!='atom']) + count(ancestor::*[name()!='void' and name()!='atom'])"/>
    <xsl:copy>
      <!-- Copy existing attributes -->
      <xsl:apply-templates select="@*"/>
      <!-- Add id attribute -->
      <xsl:attribute name="id">
        <xsl:value-of select="$position"/>
      </xsl:attribute>
      <!-- Process children -->
      <xsl:apply-templates select="node()"/>
    </xsl:copy>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
