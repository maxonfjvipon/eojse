<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="simple-runtime" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="o[starts-with(@base, 'Φ.org.eolang.')]">
    <xsl:copy>
      <xsl:apply-templates select="@* except @base"/>
      <xsl:attribute name="base" select="concat('Φ.', substring-after(@base, 'Φ.org.eolang.'))"/>
      <xsl:apply-templates select="node()"/>
    </xsl:copy>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
