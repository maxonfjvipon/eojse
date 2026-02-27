<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="flat-disps" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="dispatch">
    <xsl:copy>
      <xsl:apply-templates select="@*"/>
      <xsl:if test="*">
        <xsl:attribute name="from" select="*[1]/@id"/>
      </xsl:if>
    </xsl:copy>
    <xsl:apply-templates select="*"/>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
