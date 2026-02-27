<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" id="void" version="2.0">
  <xsl:output encoding="UTF-8" method="xml"/>
  <xsl:template match="o[@base='∅' and @name]">
    <xsl:element name="void">
      <xsl:attribute name="name" select="@name"/>
    </xsl:element>
  </xsl:template>
  <xsl:template match="node()|@*">
    <xsl:copy>
      <xsl:apply-templates select="node()|@*"/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>
