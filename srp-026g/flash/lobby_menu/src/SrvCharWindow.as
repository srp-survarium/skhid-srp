package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.char.CharWindow;
   
   public dynamic class SrvCharWindow extends CharWindow
   {
      
      public function SrvCharWindow()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_skillsTabPage_SrvCharWindow_();
      }
      
      internal function __setProp_skillsTabPage_SrvCharWindow_() : *
      {
         try
         {
            skillsTabPage["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         skillsTabPage.autoSize = "none";
         skillsTabPage.buttonWidth = 112;
         skillsTabPage.direction = "horizontal";
         skillsTabPage.enabled = true;
         skillsTabPage.enableInitCallback = false;
         skillsTabPage.focusable = true;
         skillsTabPage.itemRendererName = "SkillBranchTabCh";
         skillsTabPage.spacing = 0;
         skillsTabPage.visible = true;
         try
         {
            skillsTabPage["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
      }
   }
}

